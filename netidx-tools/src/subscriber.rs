use anyhow::{anyhow, Error, Result};
use bytes::BytesMut;
use futures::{
    channel::mpsc::{self, Receiver, Sender},
    prelude::*,
    select_biased,
    stream::{self, FusedStream},
};
use netidx::{
    config::Config,
    path::Path,
    pool::Pooled,
    resolver::Auth,
    subscriber::{Dval, Event, SubId, Subscriber, Typ, UpdatesFlags, Value},
    utils::{BatchItem, Batched},
};
use std::{
    collections::HashMap,
    io::Write,
    str::FromStr,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    runtime::Runtime,
    time,
};

#[derive(Debug, Clone)]
enum In {
    Add(String),
    Drop(String),
    Write(String, Value),
}

impl FromStr for In {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        if s.starts_with("DROP|") && s.len() > 5 {
            Ok(In::Drop(String::from(&s[5..])))
        } else if s.starts_with("ADD|") && s.len() > 4 {
            Ok(In::Add(String::from(&s[4..])))
        } else if s.starts_with("WRITE|") && s.len() > 6 {
            let mut parts = s[6..].splitn(3, "|");
            let path = String::from(
                parts.next().ok_or_else(|| anyhow!("expected | before path"))?,
            );
            let typ = parts
                .next()
                .ok_or_else(|| anyhow!("expected | before type"))?
                .parse::<Typ>()?;
            let val =
                typ.parse(parts.next().ok_or_else(|| anyhow!("expected value"))?)?;
            Ok(In::Write(path, val))
        } else {
            bail!("parse error, expected ADD, DROP, or WRITE")
        }
    }
}

pub struct BytesWriter<'a>(pub &'a mut BytesMut);

impl Write for BytesWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.extend(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Out<'a> {
    path: &'a str,
    value: Event,
}

impl<'a> Out<'a> {
    fn write(&self, to_stdout: &mut BytesMut) {
        match &self.value {
            Event::Unsubscribed => {
                to_stdout.extend_from_slice(b"Unsubscribed");
                to_stdout.extend_from_slice(b"|");
                to_stdout.extend_from_slice(self.path.as_bytes());
                to_stdout.extend_from_slice(b"\n");
            }
            Event::Update(v) => {
                to_stdout.extend_from_slice(self.path.as_bytes());
                to_stdout.extend_from_slice(b"|");
                to_stdout.extend_from_slice(match Typ::get(&v) {
                    None => b"none",
                    Some(typ) => typ.name().as_bytes(),
                });
                to_stdout.extend_from_slice(b"|");
                write!(&mut BytesWriter(to_stdout), "{}\n", v).unwrap();
            }
        }
    }
}

struct Ctx {
    sender_updates: Sender<Pooled<Vec<(SubId, Event)>>>,
    paths: HashMap<SubId, Path>,
    subscriptions: HashMap<Path, Dval>,
    subscribe_ts: HashMap<Path, Instant>,
    subscriber: Subscriber,
    requests: Box<dyn FusedStream<Item = Result<String>> + Unpin>,
    updates: Batched<Receiver<Pooled<Vec<(SubId, Event)>>>>,
    stdout: io::Stdout,
    stderr: io::Stderr,
    to_stdout: BytesMut,
    to_stderr: BytesMut,
    oneshot: bool,
    requests_finished: bool,
    subscribe_timeout: Option<Duration>,
}

impl Ctx {
    fn new(
        subscriber: Subscriber,
        no_stdin: bool,
        oneshot: bool,
        subscribe_timeout: Option<u64>,
        paths: Vec<String>,
    ) -> Self {
        let (sender_updates, updates) = mpsc::channel(100);
        Ctx {
            sender_updates,
            paths: HashMap::new(),
            subscriber,
            subscriptions: HashMap::new(),
            subscribe_ts: HashMap::new(),
            subscribe_timeout: subscribe_timeout.map(Duration::from_secs),
            requests: {
                let init = stream::iter(paths).map(|mut p| {
                    p.insert_str(0, "ADD|");
                    Ok(p)
                });
                if no_stdin {
                    Box::new(init.fuse())
                } else {
                    let stdin = Box::pin({
                        let mut stdin = BufReader::new(io::stdin()).lines();
                        async_stream::stream! {
                            loop {
                                match stdin.next_line().await {
                                    Ok(None) => break,
                                    Ok(Some(line)) => yield Ok(line),
                                    Err(e) => yield Err(Error::from(e)),
                                }
                            }
                        }
                    });
                    Box::new(init.chain(stdin))
                }
            },
            updates: Batched::new(updates, 100_000),
            stdout: io::stdout(),
            stderr: io::stderr(),
            to_stdout: BytesMut::new(),
            to_stderr: BytesMut::new(),
            oneshot,
            requests_finished: false,
        }
    }

    fn remove_subscription(&mut self, path: &str) {
        if let Some(dv) = self.subscriptions.remove(path) {
            self.subscribe_ts.remove(path);
            self.paths.remove(&dv.id());
        }
    }

    fn add_subscription(&mut self, path: Path) {
        let subscriptions = &mut self.subscriptions;
        let subscribe_ts = &mut self.subscribe_ts;
        let subscribe_timeout = self.subscribe_timeout.is_some();
        let paths = &mut self.paths;
        let subscriber = &self.subscriber;
        let sender_updates = self.sender_updates.clone();
        subscriptions.entry(path.clone()).or_insert_with(|| {
            let s = subscriber.durable_subscribe(path.clone());
            paths.insert(s.id(), path.clone());
            s.updates(
                UpdatesFlags::BEGIN_WITH_LAST | UpdatesFlags::STOP_COLLECTING_LAST,
                sender_updates,
            );
            if subscribe_timeout {
                subscribe_ts.insert(path.clone(), Instant::now());
            }
            s
        });
    }

    async fn process_request(&mut self, r: Option<Result<String>>) -> Result<()> {
        match r {
            None | Some(Err(_)) => {
                // This handles the case the user did something like
                // call us with stdin redirected from a file and we
                // read EOF, or a hereis doc, or input is piped into
                // us. We don't want to die in any of these cases.
                self.requests = Box::new(stream::pending());
                self.requests_finished = true;
                if self.oneshot && self.paths.len() == 0 {
                    self.flush().await?;
                    bail!("finished")
                } else {
                    Ok(())
                }
            }
            Some(Ok(l)) => {
                if !l.trim().is_empty() {
                    match l.parse::<In>() {
                        Err(e) => eprintln!("parse error: {}", e),
                        Ok(In::Add(p)) => self.add_subscription(Path::from(p)),
                        Ok(In::Drop(p)) => self.remove_subscription(&*p),
                        Ok(In::Write(p, v)) => {
                            let dv = match self.subscriptions.get(p.as_str()) {
                                Some(dv) => dv,
                                None => {
                                    self.add_subscription(Path::from(Arc::from(
                                        p.as_str(),
                                    )));
                                    &self.subscriptions[p.as_str()]
                                }
                            };
                            if !dv.write(v.into()) {
                                eprintln!(
                                    "WARNING: {} queued writes to {}",
                                    dv.queued_writes(),
                                    p
                                )
                            }
                        }
                    }
                }
                Ok(())
            }
        }
    }

    async fn flush(&mut self) -> Result<()> {
        if self.to_stdout.len() > 0 {
            let to_write = self.to_stdout.split().freeze();
            self.stdout.write_all(&*to_write).await?;
        }
        if self.to_stderr.len() > 0 {
            let to_write = self.to_stderr.split().freeze();
            self.stderr.write_all(&*to_write).await?;
        }
        Ok(())
    }

    async fn check_timeouts(&mut self, timeout: Duration) -> Result<()> {
        let mut failed = Vec::new();
        for (path, started) in &self.subscribe_ts {
            if started.elapsed() > timeout {
                failed.push(path.clone())
            }
        }
        for path in failed {
            eprintln!(
                "WARNING: subscription to {} did not succeed before timeout and will be canceled",
                &path
            );
            self.remove_subscription(&path)
        }
        if self.oneshot && self.paths.len() == 0 && self.requests_finished {
            self.flush().await?;
            bail!("oneshot done")
        }
        Ok(())
    }

    async fn process_update(
        &mut self,
        u: Option<BatchItem<Pooled<Vec<(SubId, Event)>>>>,
    ) -> Result<()> {
        Ok(match u {
            None => unreachable!(), // channel will never close
            Some(BatchItem::EndBatch) => self.flush().await?,
            Some(BatchItem::InBatch(mut batch)) => {
                for (id, value) in batch.drain(..) {
                    if let Some(path) = self.paths.get(&id) {
                        if self.subscribe_timeout.is_some() {
                            self.subscribe_ts.remove(path);
                        }
                        Out { path: &**path, value }.write(&mut self.to_stdout);
                        if self.oneshot {
                            if let Some(path) = self.paths.get(&id).cloned() {
                                self.remove_subscription(&path);
                            }
                            if self.paths.len() == 0 && self.requests_finished {
                                self.flush().await?;
                                bail!("oneshot finished")
                            }
                        }
                    }
                }
            }
        })
    }
}

async fn subscribe(
    cfg: Config,
    no_stdin: bool,
    oneshot: bool,
    subscribe_timeout: Option<u64>,
    paths: Vec<String>,
    auth: Auth,
) {
    let subscriber = Subscriber::new(cfg, auth).expect("create subscriber");
    let mut ctx = Ctx::new(subscriber, no_stdin, oneshot, subscribe_timeout, paths);
    let mut tick = time::interval(Duration::from_secs(1));
    loop {
        select_biased! {
            _ = tick.tick().fuse() => if let Some(timeout) = ctx.subscribe_timeout {
                match ctx.check_timeouts(timeout).await {
                    Ok(()) => (),
                    Err(_) => break,
                }
            },
            u = ctx.updates.next() => match ctx.process_update(u).await {
                Ok(()) => (),
                Err(_) => break,
            },
            r = ctx.requests.next() => match ctx.process_request(r).await {
                Ok(()) => (),
                Err(_) => break,
            },
        }
    }
}

pub(crate) fn run(
    cfg: Config,
    no_stdin: bool,
    oneshot: bool,
    subscribe_timeout: Option<u64>,
    paths: Vec<String>,
    auth: Auth,
) {
    let rt = Runtime::new().expect("failed to init runtime");
    rt.block_on(subscribe(cfg, no_stdin, oneshot, subscribe_timeout, paths, auth));
}
