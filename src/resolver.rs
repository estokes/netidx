use crate::{
    channel::Channel,
    chars::Chars,
    config::{self, resolver::Auth as CAuth},
    os::{self, ClientCtx, Krb5Ctx},
    path::Path,
    protocol::resolver::{
        self,
        v1::{
            ClientAuthRead, ClientAuthWrite, ClientHello, ClientHelloWrite, CtxId,
            FromRead, FromWrite, Resolved, ResolverId, ServerAuthWrite, ServerHelloRead,
            ServerHelloWrite, ToRead, ToWrite,
        },
    },
    utils,
};
use anyhow::{anyhow, Error, Result};
use bytes::Bytes;
use futures::{future::select_ok, prelude::*, select_biased, stream::Fuse};
use fxhash::FxBuildHasher;
use log::{debug, info, warn};
use parking_lot::RwLock;
use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    error, fmt,
    fmt::Debug,
    net::SocketAddr,
    sync::Arc,
    time::Duration,
};
use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
    task,
    time::{self, Instant, Interval},
};

#[derive(Debug, Clone)]
pub enum ResolverError {
    Denied,
    Unexpected,
    Error(Chars),
}

impl fmt::Display for ResolverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "resolver error: {:?}", self)
    }
}

impl error::Error for ResolverError {}

static TTL: u64 = 120;

#[derive(Debug, Clone)]
pub enum Auth {
    Anonymous,
    Krb5 { upn: Option<String>, spn: Option<String> },
}

type FromCon<F> = oneshot::Sender<F>;
type ToCon<T, F> = (T, FromCon<F>);

async fn send<T: Send + Sync + Debug + 'static, F: Send + Sync + Debug + 'static>(
    sender: &mpsc::UnboundedSender<ToCon<T, F>>,
    m: T,
) -> Result<F> {
    let (tx, rx) = oneshot::channel();
    sender.send((m, tx))?;
    Ok(rx.await?)
}

fn create_ctx(upn: Option<&[u8]>, target_spn: &[u8]) -> Result<(ClientCtx, Bytes)> {
    let ctx = os::create_client_ctx(upn, target_spn)?;
    match ctx.step(None)? {
        None => bail!("client ctx first step produced no token"),
        Some(tok) => Ok((ctx, utils::bytes(&*tok))),
    }
}

fn choose_read_addr(resolver: &config::resolver::Config) -> (ResolverId, SocketAddr) {
    use rand::{thread_rng, Rng};
    resolver.servers[thread_rng().gen_range(0, resolver.servers.len())]
}

async fn connect_read(
    resolver: &config::resolver::Config,
    sc: &mut Option<(CtxId, ClientCtx)>,
    desired_auth: &Auth,
) -> Result<Channel<ClientCtx>> {
    let mut backoff = 0;
    loop {
        if backoff > 0 {
            time::delay_for(Duration::from_secs(backoff)).await;
        }
        backoff += 1;
        let (rid, addr) = choose_read_addr(resolver);
        let con = try_cf!("connect", continue, TcpStream::connect(&addr).await);
        let mut con = Channel::new(con);
        try_cf!("send version", continue, con.send_one(&1u64).await);
        let _ver: u64 = try_cf!("recv version", continue, con.receive().await);
        let (auth, ctx) = match (desired_auth, &resolver.auth) {
            (Auth::Anonymous, _) => (ClientAuthRead::Anonymous, None),
            (Auth::Krb5 { .. }, CAuth::Anonymous) => bail!("authentication unavailable"),
            (Auth::Krb5 { upn, .. }, CAuth::Krb5 { target_spns }) => match sc {
                Some((id, ctx)) => (ClientAuthRead::Reuse(*id), Some(ctx.clone())),
                None => {
                    let upn = upn.as_ref().map(|s| s.as_bytes());
                    let target_spn = target_spns
                        .get(&rid)
                        .ok_or_else(|| anyhow!("no target spn for resolver {:?}", rid))?
                        .as_bytes();
                    let (ctx, tok) =
                        try_cf!("create ctx", continue, create_ctx(upn, target_spn));
                    (ClientAuthRead::Initiate(tok), Some(ctx))
                }
            },
        };
        try_cf!("hello", continue, con.send_one(&ClientHello::ReadOnly(auth)).await);
        let r: ServerHelloRead = try_cf!("hello reply", continue, con.receive().await);
        if let Some(ref ctx) = ctx {
            con.set_ctx(ctx.clone()).await
        }
        match (desired_auth, r) {
            (Auth::Anonymous, ServerHelloRead::Anonymous) => (),
            (Auth::Anonymous, _) => {
                info!("server requires authentication");
                continue;
            }
            (Auth::Krb5 { .. }, ServerHelloRead::Anonymous) => {
                info!("could not authenticate resolver server");
                continue;
            }
            (Auth::Krb5 { .. }, ServerHelloRead::Reused) => (),
            (Auth::Krb5 { .. }, ServerHelloRead::Accepted(tok, id)) => {
                let ctx = ctx.unwrap();
                try_cf!("resolver tok", continue, ctx.step(Some(&tok)));
                *sc = Some((id, ctx));
            }
        };
        break Ok(con);
    }
}

async fn connection_read(
    mut receiver: mpsc::UnboundedReceiver<ToCon<ToRead, FromRead>>,
    resolver: config::resolver::Config,
    desired_auth: Auth,
) -> Result<()> {
    let mut ctx: Option<(CtxId, ClientCtx)> = None;
    let mut con: Option<Channel<ClientCtx>> = None;
    while let Some((m, reply)) = receiver.next().await {
        let m_r = &m;
        let r = loop {
            let c = match con {
                Some(ref mut c) => c,
                None => {
                    con = Some(connect_read(&resolver, &mut ctx, &desired_auth).await?);
                    con.as_mut().unwrap()
                }
            };
            match c.send_one(m_r).await {
                Err(_) => {
                    con = None;
                }
                Ok(()) => match c.receive().await {
                    Err(_) => {
                        con = None;
                    }
                    Ok(r) => break r,
                },
            }
        };
        let _ = reply.send(r);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ResolverRead(mpsc::UnboundedSender<ToCon<ToRead, FromRead>>);

impl ResolverRead {
    pub fn new(
        resolver: config::resolver::Config,
        desired_auth: Auth,
    ) -> Result<ResolverRead> {
        let (sender, receiver) = mpsc::unbounded_channel();
        task::spawn(async move {
            let _ = connection_read(receiver, resolver, desired_auth).await;
        });
        Ok(ResolverRead(sender))
    }

    pub async fn resolve(&self, paths: Vec<Path>) -> Result<Resolved> {
        match send(&self.0, resolver::v1::ToRead::Resolve(paths)).await? {
            resolver::v1::FromRead::Resolved(r) => Ok(r),
            resolver::v1::FromRead::Denied => Err(Error::from(ResolverError::Denied)),
            resolver::v1::FromRead::Error(e) => Err(Error::from(ResolverError::Error(e))),
            _ => Err(Error::from(ResolverError::Unexpected)),
        }
    }

    pub async fn list(&self, p: Path) -> Result<Vec<Path>> {
        match send(&self.0, resolver::v1::ToRead::List(p)).await? {
            resolver::v1::FromRead::List(v) => Ok(v),
            resolver::v1::FromRead::Denied => Err(Error::from(ResolverError::Denied)),
            resolver::v1::FromRead::Error(e) => Err(Error::from(ResolverError::Error(e))),
            _ => Err(Error::from(ResolverError::Unexpected)),
        }
    }
}

async fn connect_write(
    resolver: &config::resolver::Config,
    resolver_addr: (ResolverId, SocketAddr),
    write_addr: SocketAddr,
    published: &Arc<RwLock<HashSet<Path>>>,
    ctxts: &Arc<RwLock<HashMap<ResolverId, ClientCtx, FxBuildHasher>>>,
    desired_auth: &Auth,
) -> Result<(u64, Channel<ClientCtx>)> {
    let mut backoff = 0;
    loop {
        if backoff > 0 {
            time::delay_for(Duration::from_secs(backoff)).await;
        }
        backoff += 1;
        info!("write_con connecting to resolver {:?}", resolver_addr);
        let con = try_cf!(
            "write_con connect",
            continue,
            TcpStream::connect(&resolver_addr.1).await
        );
        let mut con = Channel::new(con);
        try_cf!("send version", continue, con.send_one(&1u64).await);
        let _version: u64 = try_cf!("receive version", continue, con.receive().await);
        let (auth, ctx) = match (desired_auth, &resolver.auth) {
            (Auth::Anonymous, _) => (ClientAuthWrite::Anonymous, None),
            (Auth::Krb5 { .. }, CAuth::Anonymous) => bail!("authentication unavailable"),
            (Auth::Krb5 { upn, spn }, CAuth::Krb5 { target_spns }) => {
                match ctxts.read().get(&resolver_addr.0) {
                    Some(ctx) => (ClientAuthWrite::Reuse, Some(ctx.clone())),
                    None => {
                        let upnr = upn.as_ref().map(|s| s.as_bytes());
                        let target_spn = target_spns
                            .get(&resolver_addr.0)
                            .ok_or_else(|| {
                                anyhow!(
                                    "no target spn for resolver {:?}",
                                    resolver_addr.0
                                )
                            })?
                            .as_bytes();
                        let (ctx, token) =
                            try_cf!("ctx", continue, create_ctx(upnr, target_spn));
                        let spn = spn.as_ref().or(upn.as_ref()).cloned().map(Chars::from);
                        (ClientAuthWrite::Initiate { spn, token }, Some(ctx))
                    }
                }
            }
        };
        let h = ClientHello::WriteOnly(ClientHelloWrite { write_addr, auth });
        debug!("write_con connection established hello {:?}", h);
        try_cf!("hello", continue, con.send_one(&h).await);
        let r: ServerHelloWrite = try_cf!("hello reply", continue, con.receive().await);
        debug!("write_con resolver hello {:?}", r);
        match (desired_auth, r.auth) {
            (Auth::Anonymous, ServerAuthWrite::Anonymous) => (),
            (Auth::Anonymous, _) => {
                warn!("server requires authentication");
                continue;
            }
            (Auth::Krb5 { .. }, ServerAuthWrite::Anonymous) => {
                warn!("could not authenticate resolver server");
                continue;
            }
            (Auth::Krb5 { .. }, ServerAuthWrite::Reused) => {
                if let Some(ref ctx) = ctx {
                    con.set_ctx(ctx.clone()).await;
                    info!("write_con all traffic now encrypted");
                }
            }
            (Auth::Krb5 { .. }, ServerAuthWrite::Accepted(tok)) => {
                let ctx = ctx.unwrap();
                info!("write_con processing resolver mutual authentication");
                try_cf!("resolver tok", continue, ctx.step(Some(&tok)));
                info!("write_con mutual authentication succeeded");
                if r.resolver_id != resolver_addr.0 {
                    bail!("resolver id mismatch, bad configuration");
                }
                ctxts.write().insert(r.resolver_id, ctx.clone());
                con.set_ctx(ctx).await;
                info!("write_con all traffic now encrypted");
            }
        }
        if !r.ttl_expired {
            info!("connected to resolver {:?} for write", resolver_addr);
            break Ok((r.ttl, con));
        } else {
            let names: Vec<Path> = published.read().iter().cloned().collect();
            let len = names.len();
            if len == 0 {
                info!("connected to resolver {:?} for write", resolver_addr);
                break Ok((r.ttl, con));
            } else {
                info!("write_con ttl is expired, republishing {}", len);
                try_cf!(
                    "republish",
                    continue,
                    con.send_one(&ToWrite::Publish(names)).await
                );
                match try_cf!("replublish reply", continue, con.receive().await) {
                    FromWrite::Published => {
                        info!(
                            "connected to resolver {:?} for write (republished {})",
                            resolver_addr, len
                        );
                        break Ok((r.ttl, con));
                    }
                    r => warn!("unexpected republish reply {:?}", r),
                }
            }
        }
    }
}

async fn connection_write(
    receiver: mpsc::UnboundedReceiver<ToCon<Arc<ToWrite>, FromWrite>>,
    resolver: config::resolver::Config,
    resolver_addr: (ResolverId, SocketAddr),
    write_addr: SocketAddr,
    published: Arc<RwLock<HashSet<Path>>>,
    desired_auth: Auth,
    ctxts: Arc<RwLock<HashMap<ResolverId, ClientCtx, FxBuildHasher>>>,
) -> Result<()> {
    let mut con: Option<Channel<ClientCtx>> = None;
    let hb = Duration::from_secs(TTL / 2);
    let linger = Duration::from_secs(TTL / 10);
    let now = Instant::now();
    let mut act = false;
    let mut receiver = receiver.fuse();
    let mut hb = time::interval_at(now + hb, hb).fuse();
    let mut dc = time::interval_at(now + linger, linger).fuse();
    fn set_ttl(ttl: u64, hb: &mut Fuse<Interval>, dc: &mut Fuse<Interval>) {
        let linger = Duration::from_secs(max(1, ttl / 10));
        let heartbeat = Duration::from_secs(max(1, ttl / 2));
        let now = Instant::now();
        *hb = time::interval_at(now + heartbeat, heartbeat).fuse();
        *dc = time::interval_at(now + linger, linger).fuse();
    }
    loop {
        select_biased! {
            _ = dc.next() => {
                if act {
                   act = false;
                } else if con.is_some() {
                    info!("write_con dropping inactive connection");
                    con = None;
                }
            },
            _ = hb.next() => loop {
                if act {
                    break;
                } else {
                    match con {
                        None => {
                            let (ttl, c) = connect_write(
                                &resolver, resolver_addr, write_addr, &published,
                                &ctxts, &desired_auth
                            ).await?;
                            set_ttl(ttl, &mut hb, &mut dc);
                            con = Some(c);
                            break;
                        },
                        Some(ref mut c) => match c.send_one(&ToWrite::Heartbeat).await {
                            Ok(()) => break,
                            Err(e) => {
                                info!("write_con heartbeat send error {}", e);
                                con = None;
                            }
                        }
                    }
                }
            },
            m = receiver.next() => match m {
                None => break,
                Some((m, reply)) => {
                    act = true;
                    let m_r = &*m;
                    let r = loop {
                        let c = match con {
                            Some(ref mut c) => c,
                            None => {
                                let (ttl, c) = connect_write(
                                    &resolver, resolver_addr, write_addr, &published,
                                    &ctxts, &desired_auth
                                ).await?;
                                set_ttl(ttl, &mut hb, &mut dc);
                                con = Some(c);
                                con.as_mut().unwrap()
                            }
                        };
                        match c.send_one(m_r).await {
                            Err(e) => {
                                info!("write_con connection send error {}", e);
                                con = None;
                            }
                            Ok(()) => match c.receive().await {
                                Err(e) => {
                                    info!("write_con connection recv error {}", e);
                                    con = None;
                                }
                                Ok(r) => break r,
                            }
                        }
                    };
                    let _ = reply.send(r);
                }
            }
        }
    }
    Ok(())
}

async fn write_mgr(
    mut receiver: mpsc::UnboundedReceiver<ToCon<ToWrite, FromWrite>>,
    resolver: config::resolver::Config,
    desired_auth: Auth,
    ctxts: Arc<RwLock<HashMap<ResolverId, ClientCtx, FxBuildHasher>>>,
    write_addr: SocketAddr,
) -> Result<()> {
    let published: Arc<RwLock<HashSet<Path>>> = Arc::new(RwLock::new(HashSet::new()));
    let senders = {
        let mut senders = Vec::new();
        for addr in &resolver.servers {
            let (sender, receiver) = mpsc::unbounded_channel();
            let addr = *addr;
            let resolver = resolver.clone();
            let published = published.clone();
            let desired_auth = desired_auth.clone();
            let ctxts = ctxts.clone();
            senders.push(sender);
            task::spawn(async move {
                let r = connection_write(
                    receiver,
                    resolver,
                    addr,
                    write_addr,
                    published,
                    desired_auth.clone(),
                    ctxts.clone(),
                )
                .await;
                info!(
                    "resolver: write manager connection for {:?} exited: {:?}",
                    addr, r
                );
            });
        }
        senders
    };
    while let Some((m, reply)) = receiver.next().await {
        let m = Arc::new(m);
        let r = select_ok(senders.iter().map(|s| {
            let (tx, rx) = oneshot::channel();
            let _ = s.send((Arc::clone(&m), tx));
            rx
        }))
        .await?
        .0;
        if let ToWrite::Publish(paths) = &*m {
            match r {
                FromWrite::Published => {
                    for p in paths {
                        published.write().insert(p.clone());
                    }
                }
                _ => (),
            }
        }
        let _ = reply.send(r);
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct ResolverWrite {
    sender: mpsc::UnboundedSender<ToCon<ToWrite, FromWrite>>,
    ctxts: Arc<RwLock<HashMap<ResolverId, ClientCtx, FxBuildHasher>>>,
}

impl ResolverWrite {
    pub fn new(
        resolver: config::resolver::Config,
        desired_auth: Auth,
        write_addr: SocketAddr,
    ) -> Result<ResolverWrite> {
        let (sender, receiver) = mpsc::unbounded_channel();
        let ctxts = Arc::new(RwLock::new(HashMap::with_hasher(FxBuildHasher::default())));
        let ctxts_c = ctxts.clone();
        task::spawn(async move {
            let r =
                write_mgr(receiver, resolver, desired_auth, ctxts_c, write_addr).await;
            info!("resolver: write manager exited {:?}", r);
        });
        Ok(ResolverWrite { sender, ctxts })
    }

    pub async fn publish(&self, paths: Vec<Path>) -> Result<()> {
        match send(&self.sender, ToWrite::Publish(paths)).await? {
            FromWrite::Published => Ok(()),
            FromWrite::Denied => Err(Error::from(ResolverError::Denied)),
            FromWrite::Error(e) => Err(Error::from(ResolverError::Error(e))),
            _ => Err(Error::from(ResolverError::Unexpected)),
        }
    }

    pub async fn unpublish(&self, paths: Vec<Path>) -> Result<()> {
        match send(&self.sender, ToWrite::Unpublish(paths)).await? {
            FromWrite::Unpublished => Ok(()),
            FromWrite::Denied => Err(Error::from(ResolverError::Denied)),
            FromWrite::Error(e) => Err(Error::from(ResolverError::Error(e))),
            _ => Err(Error::from(ResolverError::Unexpected)),
        }
    }

    pub async fn clear(&self) -> Result<()> {
        match send(&self.sender, ToWrite::Clear).await? {
            FromWrite::Unpublished => Ok(()),
            FromWrite::Denied => Err(Error::from(ResolverError::Denied)),
            FromWrite::Error(e) => Err(Error::from(ResolverError::Error(e))),
            _ => Err(Error::from(ResolverError::Unexpected)),
        }
    }

    pub(crate) fn ctxts(
        &self,
    ) -> Arc<RwLock<HashMap<ResolverId, ClientCtx, FxBuildHasher>>> {
        self.ctxts.clone()
    }
}
