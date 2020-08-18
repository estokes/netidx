use crate::publisher::{parse_val, SValue, Typ};
use anyhow::{anyhow, Error, Result};
use bytes::BytesMut;
use futures::{
    channel::mpsc::{self, Receiver, Sender, UnboundedReceiver, UnboundedSender},
    prelude::*,
    select_biased,
    stream::{self, FusedStream},
};
use netidx::{
    config::Config,
    path::Path,
    pool::Pooled,
    resolver::Auth,
    subscriber::{DvState, Dval, SubId, Subscriber, Value},
    utils::{BatchItem, Batched},
};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    str::FromStr,
};
use tokio::{
    io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader},
    runtime::Runtime,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
enum In {
    Add(String),
    Drop(String),
    Write(String, SValue),
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
                parse_val(typ, parts.next().ok_or_else(|| anyhow!("expected value"))?)?;
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

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Out<'a> {
    path: &'a str,
    value: SValue,
}

impl<'a> Out<'a> {
    fn write(&self, to_stdout: &mut BytesMut) {
        to_stdout.extend_from_slice(self.path.as_bytes());
        to_stdout.extend_from_slice(b"|");
        to_stdout.extend_from_slice(match self.value.typ() {
            None => b"none",
            Some(typ) => typ.name().as_bytes(),
        });
        to_stdout.extend_from_slice(b"|");
        let mut w = BytesWriter(to_stdout);
        match &self.value {
            SValue::U32(v) | SValue::V32(v) => write!(&mut w, "{}", v),
            SValue::I32(v) | SValue::Z32(v) => write!(&mut w, "{}", v),
            SValue::U64(v) | SValue::V64(v) => write!(&mut w, "{}", v),
            SValue::I64(v) | SValue::Z64(v) => write!(&mut w, "{}", v),
            SValue::F32(v) => write!(&mut w, "{}", v),
            SValue::F64(v) => write!(&mut w, "{}", v),
            SValue::String(v) => write!(&mut w, "{}", &*v),
            SValue::Bytes(v) => write!(&mut w, "{}", &*base64::encode(v)),
            SValue::True => write!(&mut w, "true"),
            SValue::False => write!(&mut w, "false"),
            SValue::Null => write!(&mut w, "null"),
            SValue::Error(v) => write!(&mut w, "error {}", v),
        }
        .unwrap(); // this can't fail
        write!(w, "\n").unwrap();
    }
}

struct Ctx {
    sender_updates: Sender<Pooled<Vec<(SubId, Value)>>>,
    sender_states: UnboundedSender<(SubId, DvState)>,
    paths: HashMap<SubId, Path>,
    subscriptions: HashMap<Path, Dval>,
    subscriber: Subscriber,
    requests: Box<dyn FusedStream<Item = BatchItem<Result<String>>> + Unpin>,
    updates: Batched<Receiver<Pooled<Vec<(SubId, Value)>>>>,
    states: UnboundedReceiver<(SubId, DvState)>,
    stdout: io::Stdout,
    stderr: io::Stderr,
    to_stdout: BytesMut,
    to_stderr: BytesMut,
    add: HashSet<String>,
    drop: HashSet<String>,
    write: Vec<(String, SValue)>,
}

impl Ctx {
    fn new(subscriber: Subscriber, paths: Vec<String>) -> Self {
        let (sender_updates, updates) = mpsc::channel(100);
        let (sender_states, states) = mpsc::unbounded();
        Ctx {
            sender_updates,
            sender_states,
            paths: HashMap::new(),
            subscriber,
            subscriptions: HashMap::new(),
            requests: {
                let stdin = BufReader::new(io::stdin()).lines().map_err(Error::from);
                Box::new(Batched::new(
                    stream::iter(paths)
                        .map(|mut p| {
                            p.insert_str(0, "ADD|");
                            Ok(p)
                        })
                        .chain(stdin),
                    1000,
                ))
            },
            updates: Batched::new(updates, 100_000),
            states,
            stdout: io::stdout(),
            stderr: io::stderr(),
            to_stdout: BytesMut::new(),
            to_stderr: BytesMut::new(),
            add: HashSet::new(),
            drop: HashSet::new(),
            write: Vec::new(),
        }
    }

    async fn process_request(&mut self, r: Option<BatchItem<Result<String>>>) {
        match r {
            None | Some(BatchItem::InBatch(Err(_))) => {
                // This handles the case the user did something like
                // call us with stdin redirected from a file and we
                // read EOF, or a hereis doc, or input is piped into
                // us. We don't want to die in any of these cases.
                self.requests = Box::new(stream::pending());
            }
            Some(BatchItem::InBatch(Ok(l))) => {
                if !l.trim().is_empty() {
                    match l.parse::<In>() {
                        Err(e) => eprintln!("parse error: {}", e),
                        Ok(In::Add(p)) => {
                            if !self.drop.remove(&p) {
                                self.add.insert(p);
                            }
                        }
                        Ok(In::Drop(p)) => {
                            if !self.add.remove(&p) {
                                self.drop.insert(p);
                            }
                        }
                        Ok(In::Write(p, v)) => {
                            self.write.push((p, v));
                        }
                    }
                }
            }
            Some(BatchItem::EndBatch) => {
                for p in self.drop.drain() {
                    if let Some(sub) = self.subscriptions.remove(&*p) {
                        self.paths.remove(&sub.id());
                    }
                }
                for p in self.add.drain() {
                    let p = Path::from(p);
                    let subscriptions = &mut self.subscriptions;
                    let paths = &mut self.paths;
                    let subscriber = &self.subscriber;
                    let sender_updates = self.sender_updates.clone();
                    let sender_states = self.sender_states.clone();
                    subscriptions.entry(p.clone()).or_insert_with(|| {
                        let s = subscriber.durable_subscribe(p.clone());
                        paths.insert(s.id(), p.clone());
                        s.updates(true, sender_updates);
                        s.state_updates(true, sender_states);
                        s
                    });
                }
                self.subscriber.flush().await;
                for (p, v) in self.write.drain(..) {
                    match self.subscriptions.get(p.as_str()) {
                        None => {
                            eprintln!("write to unknown path {}, subscribe first?", p)
                        }
                        Some(dv) => match dv.state() {
                            DvState::Subscribed => {
                                dv.write(v.into());
                            }
                            DvState::Unsubscribed | DvState::FatalError(_) => eprintln!(
                                "write to failed subscription {} ignored retry later?",
                                p
                            ),
                        },
                    }
                }
            }
        }
    }

    async fn process_update(
        &mut self,
        u: Option<BatchItem<Pooled<Vec<(SubId, Value)>>>>,
    ) -> Result<()> {
        Ok(match u {
            None => unreachable!(), // channel will never close
            Some(BatchItem::EndBatch) => {
                if self.to_stdout.len() > 0 {
                    let to_write = self.to_stdout.split().freeze();
                    self.stdout.write_all(&*to_write).await?;
                }
                if self.to_stderr.len() > 0 {
                    let to_write = self.to_stderr.split().freeze();
                    self.stderr.write_all(&*to_write).await?;
                }
            }
            Some(BatchItem::InBatch(mut batch)) => {
                for (id, value) in batch.drain(..) {
                    if let Some(path) = self.paths.get(&id) {
                        let value = SValue::from(value);
                        Out { path: &**path, value }.write(&mut self.to_stdout);
                    }
                }
            }
        })
    }

    fn process_state_change(&mut self, u: Option<(SubId, DvState)>) {
        match u {
            None => unreachable!(), // channel will never close
            Some((id, DvState::Unsubscribed)) => eprintln!(
                "subscription to {} now Unsubscribed",
                self.paths.get(&id).map(|p| p.as_ref()).unwrap_or("{unknown}")
            ),
            Some((id, DvState::Subscribed)) => eprintln!(
                "subscription to {} now Subscribed",
                self.paths.get(&id).map(|p| p.as_ref()).unwrap_or("{unknown}")
            ),
            Some((id, DvState::FatalError(e))) => {
                if let Some(path) = self.paths.remove(&id) {
                    self.subscriptions.remove(&path);
                    eprintln!("subscription to {} is dead {}", path, e);
                }
            }
        }
    }
}

async fn subscribe(cfg: Config, paths: Vec<String>, auth: Auth) {
    let subscriber = Subscriber::new(cfg, auth).expect("create subscriber");
    let mut ctx = Ctx::new(subscriber, paths);
    loop {
        select_biased! {
            u = ctx.updates.next() => {
                match ctx.process_update(u).await {
                    Ok(()) => (),
                    Err(_) => break,
                }
            },
            u = ctx.states.next() => ctx.process_state_change(u),
            r = ctx.requests.next() => ctx.process_request(r).await
        }
    }
}

pub(crate) fn run(cfg: Config, paths: Vec<String>, auth: Auth) {
    let mut rt = Runtime::new().expect("failed to init runtime");
    rt.block_on(subscribe(cfg, paths, auth));
}
