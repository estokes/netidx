use crate::{
    chars::Chars,
    pack::{self, Pack, PackError},
    path::Path,
};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use std::{
    fmt, mem,
    net::SocketAddr,
    ops::{Add, Sub, Div, Mul, Not},
    result,
};

type Result<T> = result::Result<T, PackError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u64);

impl Id {
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Id(NEXT.fetch_add(1, Ordering::Relaxed))
    }

    #[cfg(test)]
    pub(crate) fn mk(i: u64) -> Self {
        Id(i)
    }
}

impl Pack for Id {
    fn len(&self) -> usize {
        pack::varint_len(self.0)
    }

    fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        Ok(pack::encode_varint(self.0, buf))
    }

    fn decode(buf: &mut BytesMut) -> Result<Self> {
        Ok(Id(pack::decode_varint(buf)?))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hello {
    /// No authentication will be provided. The publisher may drop
    /// the connection at this point, if it chooses to allow this
    /// then it will return Anonymous.
    Anonymous,
    /// An authentication token, if the token is valid then the
    /// publisher will send a token back to authenticate itself to
    /// the subscriber.
    Token(Bytes),
    /// In order to prevent denial of service, spoofing, etc,
    /// authenticated publishers must prove that they are actually
    /// listening on the socket they claim to be listening on. To
    /// facilitate this, after a new security context has been
    /// created the resolver server will encrypt a random number
    /// with it, connect to the write address specified by the
    /// publisher, and send the encrypted token. The publisher
    /// must decrypt the token using it's end of the security
    /// context, add 1 to the number, encrypt it again and send it
    /// back. If that round trip succeeds then the new security
    /// context will replace any old one, if it fails the new
    /// context will be thrown away and the old one will continue
    /// to be associated with the write address.
    ResolverAuthenticate(SocketAddr, Bytes),
}

impl Pack for Hello {
    fn len(&self) -> usize {
        1 + match self {
            Hello::Anonymous => 0,
            Hello::Token(tok) => <Bytes as Pack>::len(tok),
            Hello::ResolverAuthenticate(addr, tok) => {
                <SocketAddr as Pack>::len(addr) + <Bytes as Pack>::len(tok)
            }
        }
    }

    fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        match self {
            Hello::Anonymous => Ok(buf.put_u8(0)),
            Hello::Token(tok) => {
                buf.put_u8(1);
                <Bytes as Pack>::encode(tok, buf)
            }
            Hello::ResolverAuthenticate(id, tok) => {
                buf.put_u8(2);
                <SocketAddr as Pack>::encode(id, buf)?;
                <Bytes as Pack>::encode(tok, buf)
            }
        }
    }

    fn decode(buf: &mut BytesMut) -> Result<Self> {
        match buf.get_u8() {
            0 => Ok(Hello::Anonymous),
            1 => Ok(Hello::Token(<Bytes as Pack>::decode(buf)?)),
            2 => {
                let addr = <SocketAddr as Pack>::decode(buf)?;
                let tok = <Bytes as Pack>::decode(buf)?;
                Ok(Hello::ResolverAuthenticate(addr, tok))
            }
            _ => Err(PackError::UnknownTag),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum To {
    /// Subscribe to the specified value, if it is not available
    /// the result will be NoSuchValue. The optional security
    /// token is a proof from the resolver server that this
    /// subscription is permitted. In the case of an anonymous
    /// connection this proof will be empty.
    Subscribe {
        path: Path,
        resolver: SocketAddr,
        timestamp: u64,
        permissions: u32,
        token: Bytes,
    },
    /// Unsubscribe from the specified value, this will always result
    /// in an Unsubscibed message even if you weren't ever subscribed
    /// to the value, or it doesn't exist.
    Unsubscribe(Id),
    /// Send a write to the specified value.
    Write(Id, Value, bool),
}

impl Pack for To {
    fn len(&self) -> usize {
        1 + match self {
            To::Subscribe { path, resolver, timestamp, permissions, token } => {
                <Path as Pack>::len(path)
                    + <SocketAddr as Pack>::len(resolver)
                    + <u64 as Pack>::len(timestamp)
                    + <u32 as Pack>::len(permissions)
                    + <Bytes as Pack>::len(token)
            }
            To::Unsubscribe(id) => Id::len(id),
            To::Write(id, v, reply) => {
                Id::len(id) + Value::len(v) + <bool as Pack>::len(reply)
            }
        }
    }

    fn encode(&self, buf: &mut BytesMut) -> anyhow::Result<(), PackError> {
        match self {
            To::Subscribe { path, resolver, timestamp, permissions, token } => {
                buf.put_u8(0);
                <Path as Pack>::encode(path, buf)?;
                <SocketAddr as Pack>::encode(resolver, buf)?;
                <u64 as Pack>::encode(timestamp, buf)?;
                <u32 as Pack>::encode(permissions, buf)?;
                <Bytes as Pack>::encode(token, buf)
            }
            To::Unsubscribe(id) => {
                buf.put_u8(1);
                Id::encode(id, buf)
            }
            To::Write(id, v, reply) => {
                buf.put_u8(2);
                Id::encode(id, buf)?;
                Value::encode(v, buf)?
                <bool as Pack>::encode(reply, buf)
            }
        }
    }

    fn decode(buf: &mut BytesMut) -> anyhow::Result<Self, PackError> {
        match buf.get_u8() {
            0 => {
                let path = <Path as Pack>::decode(buf)?;
                let resolver = <SocketAddr as Pack>::decode(buf)?;
                let timestamp = <u64 as Pack>::decode(buf)?;
                let permissions = <u32 as Pack>::decode(buf)?;
                let token = <Bytes as Pack>::decode(buf)?;
                Ok(To::Subscribe { path, resolver, timestamp, permissions, token })
            }
            1 => Ok(To::Unsubscribe(Id::decode(buf)?)),
            2 => {
                let id = Id::decode(buf)?;
                let v = Value::decode(buf)?;
                let reply = <bool as Pack>::decode(buf)?;
                Ok(To::Write(id, v, reply))
            }
            _ => Err(PackError::UnknownTag),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum Value {
    /// full 4 byte u32
    U32(u32),
    /// LEB128 varint, 1 - 5 bytes depending on value
    V32(u32),
    /// full 4 byte i32
    I32(i32),
    /// LEB128 varint zigzag encoded, 1 - 5 bytes depending on abs(value)
    Z32(i32),
    /// full 8 byte u64
    U64(u64),
    /// LEB128 varint, 1 - 10 bytes depending on value
    V64(u64),
    /// full 8 byte i64
    I64(i64),
    /// LEB128 varint zigzag encoded, 1 - 10 bytes depending on abs(value)
    Z64(i64),
    /// 4 byte ieee754 single precision float
    F32(f32),
    /// 8 byte ieee754 double precision float
    F64(f64),
    /// unicode string, zero copy decode
    String(Chars),
    /// byte array, zero copy decode
    Bytes(Bytes),
    /// boolean true
    True,
    /// boolean false
    False,
    /// Empty value
    Null,
    /// An explicit ok
    Ok,
    /// An explicit error
    Error(Chars),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::U32(v) | Value::V32(v) => write!(f, "{}", v),
            Value::I32(v) | Value::Z32(v) => write!(f, "{}", v),
            Value::U64(v) | Value::V64(v) => write!(f, "{}", v),
            Value::I64(v) | Value::Z64(v) => write!(f, "{}", v),
            Value::F32(v) => write!(f, "{}", v),
            Value::F64(v) => write!(f, "{}", v),
            Value::String(v) => write!(f, "{}", &*v),
            Value::Bytes(_) => write!(f, "<binary>"),
            Value::True => write!(f, "True"),
            Value::False => write!(f, "False"),
            Value::Null => write!(f, "Null"),
            Value::Ok => write!(f, "Ok"),
            Value::Error(v) => write!(f, "Error {}", v),
        }
    }
}

impl Add for Value {
    type Output = Value;

    fn add(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Value::U32(l), Value::U32(r)) => Value::U32(l + r),
            (Value::U32(l), Value::V32(r)) => Value::U32(l + r),
            (Value::V32(l), Value::V32(r)) => Value::V32(l + r),
            (Value::V32(l), Value::U32(r)) => Value::U32(l + r),
            (Value::I32(l), Value::I32(r)) => Value::I32(l + r),
            (Value::I32(l), Value::Z32(r)) => Value::I32(l + r),
            (Value::Z32(l), Value::Z32(r)) => Value::Z32(l + r),
            (Value::Z32(l), Value::I32(r)) => Value::I32(l + r),
            (Value::U64(l), Value::U64(r)) => Value::U64(l + r),
            (Value::U64(l), Value::V64(r)) => Value::U64(l + r),
            (Value::V64(l), Value::V64(r)) => Value::V64(l + r),
            (Value::I64(l), Value::I64(r)) => Value::I64(l + r),
            (Value::I64(l), Value::Z64(r)) => Value::I64(l + r),
            (Value::Z64(l), Value::Z64(r)) => Value::Z64(l + r),
            (Value::Z64(l), Value::I64(r)) => Value::I64(l + r),
            (Value::F32(l), Value::F32(r)) => Value::F32(l + r),
            (Value::F64(l), Value::F64(r)) => Value::F64(l + r),
            (l, r) => Value::Error(Chars::from(format!("can't add {:?} and {:?}", l, r))),
        }
    }
}

impl Sub for Value {
    type Output = Value;

    fn sub(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Value::U32(l), Value::U32(r)) if l >= r => Value::U32(l - r),
            (Value::U32(l), Value::V32(r)) if l >= r => Value::U32(l - r),
            (Value::V32(l), Value::V32(r)) if l >= r => Value::V32(l - r),
            (Value::V32(l), Value::U32(r)) if l >= r => Value::U32(l - r),
            (Value::I32(l), Value::I32(r)) => Value::I32(l - r),
            (Value::I32(l), Value::Z32(r)) => Value::I32(l - r),
            (Value::Z32(l), Value::Z32(r)) => Value::Z32(l - r),
            (Value::Z32(l), Value::I32(r)) => Value::I32(l - r),
            (Value::U64(l), Value::U64(r)) if l >= r => Value::U64(l - r),
            (Value::U64(l), Value::V64(r)) if l >= r => Value::U64(l - r),
            (Value::V64(l), Value::V64(r)) if l >= r => Value::V64(l - r),
            (Value::I64(l), Value::I64(r)) => Value::I64(l - r),
            (Value::I64(l), Value::Z64(r)) => Value::I64(l - r),
            (Value::Z64(l), Value::Z64(r)) => Value::Z64(l - r),
            (Value::Z64(l), Value::I64(r)) => Value::I64(l - r),
            (Value::F32(l), Value::F32(r)) => Value::F32(l - r),
            (Value::F64(l), Value::F64(r)) => Value::F64(l - r),
            (l, r) => Value::Error(Chars::from(format!("can't sub {:?} and {:?}", l, r))),
        }
    }
}

impl Mul for Value {
    type Output = Value;

    fn mul(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Value::U32(l), Value::U32(r)) => Value::U32(l * r),
            (Value::U32(l), Value::V32(r)) => Value::U32(l * r),
            (Value::V32(l), Value::V32(r)) => Value::V32(l * r),
            (Value::V32(l), Value::U32(r)) => Value::U32(l * r),
            (Value::I32(l), Value::I32(r)) => Value::I32(l * r),
            (Value::I32(l), Value::Z32(r)) => Value::I32(l * r),
            (Value::Z32(l), Value::Z32(r)) => Value::Z32(l * r),
            (Value::Z32(l), Value::I32(r)) => Value::I32(l * r),
            (Value::U64(l), Value::U64(r)) => Value::U64(l * r),
            (Value::U64(l), Value::V64(r)) => Value::U64(l * r),
            (Value::V64(l), Value::V64(r)) => Value::V64(l * r),
            (Value::I64(l), Value::I64(r)) => Value::I64(l * r),
            (Value::I64(l), Value::Z64(r)) => Value::I64(l * r),
            (Value::Z64(l), Value::Z64(r)) => Value::Z64(l * r),
            (Value::Z64(l), Value::I64(r)) => Value::I64(l * r),
            (Value::F32(l), Value::F32(r)) => Value::F32(l * r),
            (Value::F64(l), Value::F64(r)) => Value::F64(l * r),
            (l, r) => {
                Value::Error(Chars::from(format!("can't multiply {:?} and {:?}", l, r)))
            }
        }
    }
}

impl Div for Value {
    type Output = Value;

    fn div(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Value::U32(l), Value::U32(r)) if r > 0 => Value::U32(l / r),
            (Value::U32(l), Value::V32(r)) if r > 0 => Value::U32(l / r),
            (Value::V32(l), Value::V32(r)) if r > 0 => Value::V32(l / r),
            (Value::V32(l), Value::U32(r)) if r > 0 => Value::U32(l / r),
            (Value::I32(l), Value::I32(r)) if r > 0 => Value::I32(l / r),
            (Value::I32(l), Value::Z32(r)) if r > 0 => Value::I32(l / r),
            (Value::Z32(l), Value::Z32(r)) if r > 0 => Value::Z32(l / r),
            (Value::Z32(l), Value::I32(r)) if r > 0 => Value::I32(l / r),
            (Value::U64(l), Value::U64(r)) if r > 0 => Value::U64(l / r),
            (Value::U64(l), Value::V64(r)) if r > 0 => Value::U64(l / r),
            (Value::V64(l), Value::V64(r)) if r > 0 => Value::V64(l / r),
            (Value::I64(l), Value::I64(r)) if r > 0 => Value::I64(l / r),
            (Value::I64(l), Value::Z64(r)) if r > 0 => Value::I64(l / r),
            (Value::Z64(l), Value::Z64(r)) if r > 0 => Value::Z64(l / r),
            (Value::Z64(l), Value::I64(r)) if r > 0 => Value::I64(l / r),
            (Value::F32(l), Value::F32(r)) => Value::F32(l / r),
            (Value::F64(l), Value::F64(r)) => Value::F64(l / r),
            (l, r) => {
                Value::Error(Chars::from(format!("can't multiply {:?} and {:?}", l, r)))
            }
        }
    }
}

impl Not for Value {
    type Output = Value;

    fn not(self) -> Self {
        match self {
            Value::U32(v) => {
                Value::Error(Chars::from(format!("can't apply not to U32({})", v)))
            }
            Value::V32(v) => {
                Value::Error(Chars::from(format!("can't apply not to V32({})", v)))
            }
            Value::I32(v) => {
                Value::Error(Chars::from(format!("can't apply not to I32({})", v)))
            }
            Value::Z32(v) => {
                Value::Error(Chars::from(format!("can't apply not to Z32({})", v)))
            }
            Value::U64(v) => {
                Value::Error(Chars::from(format!("can't apply not to U64({})", v)))
            }
            Value::V64(v) => {
                Value::Error(Chars::from(format!("can't apply not to V64({})", v)))
            }
            Value::I64(v) => {
                Value::Error(Chars::from(format!("can't apply not to I64({})", v)))
            }
            Value::Z64(v) => {
                Value::Error(Chars::from(format!("can't apply not to Z64({})", v)))
            }
            Value::F32(v) => {
                Value::Error(Chars::from(format!("can't apply not to F32({})", v)))
            }
            Value::F64(v) => {
                Value::Error(Chars::from(format!("can't apply not to F64({})", v)))
            }
            Value::String(v) => {
                Value::Error(Chars::from(format!("can't apply not to String({})", v)))
            }
            Value::Bytes(_) => {
                Value::Error(Chars::from(format!("can't apply not to Bytes")))
            }
            Value::True => Value::False,
            Value::False => Value::True,
            Value::Null => Value::Null,
            Value::Ok => Value::Error(Chars::from(format!("can't apply not to Ok"))),
            Value::Error(v) => {
                Value::Error(Chars::from(format!("can't apply not to Error({})", v)))
            }
        }
    }
}

impl Pack for Value {
    fn len(&self) -> usize {
        1 + match self {
            Value::U32(_) => mem::size_of::<u32>(),
            Value::V32(v) => pack::varint_len(*v as u64),
            Value::I32(_) => mem::size_of::<i32>(),
            Value::Z32(v) => pack::varint_len(pack::i32_zz(*v) as u64),
            Value::U64(_) => mem::size_of::<u64>(),
            Value::V64(v) => pack::varint_len(*v),
            Value::I64(_) => mem::size_of::<i64>(),
            Value::Z64(v) => pack::varint_len(pack::i64_zz(*v) as u64),
            Value::F32(_) => mem::size_of::<f32>(),
            Value::F64(_) => mem::size_of::<f64>(),
            Value::String(c) => <Chars as Pack>::len(c),
            Value::Bytes(b) => <Bytes as Pack>::len(b),
            Value::True | Value::False | Value::Null => 0,
            Value::Ok => 0,
            Value::Error(c) => <Chars as Pack>::len(c),
        }
    }

    fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        match self {
            Value::U32(i) => {
                buf.put_u8(0);
                Ok(buf.put_u32(*i))
            }
            Value::V32(i) => {
                buf.put_u8(1);
                Ok(pack::encode_varint(*i as u64, buf))
            }
            Value::I32(i) => {
                buf.put_u8(2);
                Ok(buf.put_i32(*i))
            }
            Value::Z32(i) => {
                buf.put_u8(3);
                Ok(pack::encode_varint(pack::i32_zz(*i) as u64, buf))
            }
            Value::U64(i) => {
                buf.put_u8(4);
                Ok(buf.put_u64(*i))
            }
            Value::V64(i) => {
                buf.put_u8(5);
                Ok(pack::encode_varint(*i, buf))
            }
            Value::I64(i) => {
                buf.put_u8(6);
                Ok(buf.put_i64(*i))
            }
            Value::Z64(i) => {
                buf.put_u8(7);
                Ok(pack::encode_varint(pack::i64_zz(*i), buf))
            }
            Value::F32(i) => {
                buf.put_u8(8);
                Ok(buf.put_f32(*i))
            }
            Value::F64(i) => {
                buf.put_u8(9);
                Ok(buf.put_f64(*i))
            }
            Value::String(s) => {
                buf.put_u8(10);
                <Chars as Pack>::encode(s, buf)
            }
            Value::Bytes(b) => {
                buf.put_u8(11);
                <Bytes as Pack>::encode(b, buf)
            }
            Value::True => Ok(buf.put_u8(12)),
            Value::False => Ok(buf.put_u8(13)),
            Value::Null => Ok(buf.put_u8(14)),
            Value::Ok => Ok(buf.put_u8(15)),
            Value::Error(e) => {
                buf.put_u8(16);
                <Chars as Pack>::encode(e, buf)
            }
        }
    }

    fn decode(buf: &mut BytesMut) -> Result<Self> {
        match buf.get_u8() {
            0 => Ok(Value::U32(buf.get_u32())),
            1 => Ok(Value::V32(pack::decode_varint(buf)? as u32)),
            2 => Ok(Value::I32(buf.get_i32())),
            3 => Ok(Value::Z32(pack::i32_uzz(pack::decode_varint(buf)? as u32))),
            4 => Ok(Value::U64(buf.get_u64())),
            5 => Ok(Value::V64(pack::decode_varint(buf)?)),
            6 => Ok(Value::I64(buf.get_i64())),
            7 => Ok(Value::Z64(pack::i64_uzz(pack::decode_varint(buf)?))),
            8 => Ok(Value::F32(buf.get_f32())),
            9 => Ok(Value::F64(buf.get_f64())),
            10 => Ok(Value::String(<Chars as Pack>::decode(buf)?)),
            11 => Ok(Value::Bytes(<Bytes as Pack>::decode(buf)?)),
            12 => Ok(Value::True),
            13 => Ok(Value::False),
            14 => Ok(Value::Null),
            15 => Ok(Value::Ok),
            16 => Ok(Value::Error(<Chars as Pack>::decode(buf)?)),
            _ => Err(PackError::UnknownTag),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum From {
    /// The requested subscription to Path cannot be completed because
    /// it doesn't exist
    NoSuchValue(Path),
    /// Permission to subscribe to the specified path is denied.
    Denied(Path),
    /// You have been unsubscriped from Path. This can be the result
    /// of an Unsubscribe message, or it may be sent unsolicited, in
    /// the case the value is no longer published, or the publisher is
    /// in the process of shutting down.
    Unsubscribed(Id),
    /// You are now subscribed to Path with subscription id `Id`, and
    /// The next message contains the first value for Id. All further
    /// communications about this subscription will only refer to the
    /// Id.
    Subscribed(Path, Id, Value),
    /// A value update to Id
    Update(Id, Value),
    /// Indicates that the publisher is idle, but still
    /// functioning correctly.
    Heartbeat,
    /// Indicates the result of a write request
    WriteResult(Id, Value),
}

impl Pack for From {
    fn len(&self) -> usize {
        1 + match self {
            From::NoSuchValue(p) => <Path as Pack>::len(p),
            From::Denied(p) => <Path as Pack>::len(p),
            From::Unsubscribed(id) => Id::len(id),
            From::Subscribed(p, id, v) => {
                <Path as Pack>::len(p) + Id::len(id) + Value::len(v)
            }
            From::Update(id, v) => Id::len(id) + Value::len(v),
            From::Heartbeat => 0,
            From::WriteResult(id, v) => Id::len(id) + Value::len(v),
        }
    }

    fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        match self {
            From::NoSuchValue(p) => {
                buf.put_u8(0);
                <Path as Pack>::encode(p, buf)
            }
            From::Denied(p) => {
                buf.put_u8(1);
                <Path as Pack>::encode(p, buf)
            }
            From::Unsubscribed(id) => {
                buf.put_u8(2);
                Id::encode(id, buf)
            }
            From::Subscribed(p, id, v) => {
                buf.put_u8(3);
                <Path as Pack>::encode(p, buf)?;
                Id::encode(id, buf)?;
                Value::encode(v, buf)
            }
            From::Update(id, v) => {
                buf.put_u8(4);
                Id::encode(id, buf)?;
                Value::encode(v, buf)
            }
            From::Heartbeat => Ok(buf.put_u8(5)),
            From::WriteResult(id, v) => {
                buf.put_u8(6);
                Id::encode(id, buf)?;
                Value::encode(v, buf)
            }
        }
    }

    fn decode(buf: &mut BytesMut) -> Result<Self> {
        match buf.get_u8() {
            0 => Ok(From::NoSuchValue(<Path as Pack>::decode(buf)?)),
            1 => Ok(From::Denied(<Path as Pack>::decode(buf)?)),
            2 => Ok(From::Unsubscribed(Id::decode(buf)?)),
            3 => {
                let path = <Path as Pack>::decode(buf)?;
                let id = Id::decode(buf)?;
                let v = Value::decode(buf)?;
                Ok(From::Subscribed(path, id, v))
            }
            4 => {
                let id = Id::decode(buf)?;
                let value = Value::decode(buf)?;
                Ok(From::Update(id, value))
            }
            5 => Ok(From::Heartbeat),
            6 => {
                let id = Id::decode(buf)?;
                let value = Value::decode(buf)?;
                Ok(From::WriteResult(id, value))
            }
            _ => Err(PackError::UnknownTag),
        }
    }
}
