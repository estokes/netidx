use json_pubsub::{
    path::Path,
    subscriber::{Subscriber, RawSubscription},
    utils::{Batched, BatchItem},
};
use futures::{
    stream, future,
    future::FutureExt as FrsFutureExt,
    sink::SinkExt as FrsSinkExt,
    channel::{oneshot, mpsc}
};
use async_std::{
    prelude::*,
    task,
    io::{self, BufReader},
};
use std::{
    mem,
    collections::{HashMap, HashSet},
    str::FromStr,
    result::Result
};
use bytes::Bytes;
use super::ResolverConfig;

fn mpv_to_json(v: rmpv::Value) -> serde_json::Value {
    use rmpv::{Value as Rv, Utf8String};
    use serde_json::{Value as Jv, Number, Map};
    use std::num::FpCategory;
    let cvt_str = |s: Utf8String| -> String {
        String::from_utf8_lossy(s.as_bytes()).into()
    };
    let cvt_float = |f: f64| -> serde_json::Value {
        match Number::from_f64(f) {
            Some(n) => Jv::Number(n),
            None => match f64::classify(f) {
                FpCategory::Nan => Jv::String("NaN".into()),
                FpCategory::Infinite => Jv::String("Infinite".into()),
                FpCategory::Zero | FpCategory::Subnormal | FpCategory::Normal =>
                    unreachable!("float should convert"),
            }
        }
    };
    match v {
        Rv::Nil => Jv::Null,
        Rv::Boolean(b) => Jv::Bool(b),
        Rv::F32(f) => cvt_float(f as f64),
        Rv::F64(f) => cvt_float(f),
        Rv::String(s) => Jv::String(cvt_str(s)),
        Rv::Binary(_) => Jv::String("<binary>".into()),
        Rv::Array(a) => Jv::Array(a.into_iter().map(mpv_to_json).collect()),
        Rv::Ext(u, _) => Jv::String(format!("extension: {} val: <binary>", u)),
        Rv::Integer(i) => {
            if let Some(i) = i.as_i64() {
                Jv::Number(Number::from(i))
            } else if let Some(u) = i.as_u64() {
                Jv::Number(Number::from(u))
            } else if let Some(f) = i.as_f64() {
                cvt_float(f)
            } else {
                unreachable!("invalid number")
            }
        }
        Rv::Map(vals) => {
            let mut m = Map::new();
            for (k, v) in vals {
                match k {
                    Rv::String(s) => {
                        m.insert(cvt_str(s), mpv_to_json(v));
                    },
                    _ => {
                        let k = serde_json::to_string(&mpv_to_json(k)).unwrap();
                        m.insert(k, mpv_to_json(v));
                    }
                }
            }
            Jv::Object(m)
        }
    }
}

async fn run_subscription(
    path: Path,
    sub: RawSubscription,
    stop: oneshot::Receiver<()>,
    mut out: mpsc::Sender<(Path, String)>,
) {
    enum M { Update(Option<Bytes>), Stop };
    let path = Path::from(path.replace('|', r"\|"));
    let mut updates = sub.updates(true);
    let stop = stop.shared();
    loop {
        let update = updates.next().map(|m| M::Update(m));
        let stop = stop.clone().map(|_| M::Stop);
        match update.race(stop).await {
            M::Stop | M::Update(None) => { break; },
            M::Update(Some(m)) => {
                let v = match rmpv::decode::value::read_value(&mut &*m) {
                    Err(_) => String::from(String::from_utf8_lossy(&*m)),
                    Ok(v) => try_brk!("json", serde_json::to_string(&mpv_to_json(v))),
                };
                try_brk!("subscription ended", out.send((path.clone(), v)).await);
            }
        }
    }
}

async fn output_vals(mut msgs: mpsc::Receiver<(Path, String)>) {
    let stdout = io::stdout();
    let mut w = stdout.lock().await;
    let mut buf = String::new();
    while let Some((path, v)) = msgs.next().await {
        buf.push_str(&*path);
        buf.push('|');
        buf.push_str(&*v);
        try_brk!("write", w.write_all(buf.as_bytes()).await);
        buf.clear();
    }
}

enum Req {
    Add(Path),
    Drop(Path),
}

impl FromStr for Req {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("drop: ") && s.len() > 6 {
            Ok(Req::Drop(Path::from(&s[6..])))
        } else {
            let p = Path::from(s);
            if !p.is_absolute() {
                Err(format!("path is not absolute {}", p))
            } else {
                Ok(Req::Add(p))
            }
        }
    }
}

pub(crate) fn run(cfg: ResolverConfig, paths: Vec<String>) {
    task::block_on(async {
        let mut subscriptions: HashMap::<Path, oneshot::Sender<()>> = HashMap::new();
        let subscriber = Subscriber::new(cfg.bind).expect("create subscriber");
        let (out_tx, out_rx) = mpsc::channel(100);
        task::spawn(output_vals(out_rx));
        let stdin = BufReader::new(io::stdin()).lines();
        let mut requests = Batched::new(
            stream::iter(paths).map(|p| Ok(p)).chain(stdin), 1000
        );
        let mut add = HashSet::new();
        let mut drop = HashSet::new();
        while let Some(l) = requests.next().await {
            match l {
                BatchItem::InBatch(l) => match try_brk!("reading", l).parse::<Req>() {
                    Err(e) => eprintln!("{}", e),
                    Ok(Req::Add(p)) => {
                        if !drop.remove(&p) {
                            add.insert(p);
                        }
                    }
                    Ok(Req::Drop(p)) => {
                        if !add.remove(&p) {
                            drop.insert(p);
                        }
                    }
                }
                BatchItem::EndBatch => {
                    for p in drop.drain() {
                        subscriptions.remove(&p).into_iter().for_each(|s| {
                            let _ = s.send(());
                        })
                    }
                    for (p, r) in subscriber.subscribe_raw(mem::take(&mut add)).await {
                        match r {
                            Err(e) => eprintln!("subscription failed: {}, {}", p, e),
                            Ok(s) => {
                                let (tx, rx) = oneshot::channel();
                                task::spawn(run_subscription(
                                    p.clone(), s, rx, out_tx.clone()
                                ));
                                subscriptions.insert(p, tx);
                            }
                        }
                    }
                }
            }
        }
        // run until we are killed even if stdin closes
        future::pending::<()>().await;
        mem::drop(subscriber);
    });
}