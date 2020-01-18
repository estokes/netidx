use crate::utils::{BytesWriter, mp_encode};
use byteorder::{BigEndian, ByteOrder};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use tokio::prelude::*;
use futures::prelude::*;
use serde::{de::DeserializeOwned, Serialize};
use std::{
    io::{Error, ErrorKind, IoSlice},
    mem,
    result::Result,
    collections::VecDeque,
    cmp::min,
};
use tokio::net::TcpStream;

const BUF: usize = 4096;

struct BytesDeque(VecDeque<Bytes>);

impl Buf for BytesDeque {
    fn remaining(&self) -> usize {
        self.0.iter().fold(0, |s, b| s + b.remaining())
    }

    fn bytes(&self) -> &[u8] {
        if self.0.len() == 0 {
            &[]
        } else {
            self.0[0].bytes()
        }
    }
    
    fn advance(&mut self, cnt: usize) {
        let mut cnt = cnt;
        while cnt > 0 && self.0.len() > 0 {
            let b = &mut self.0[0];
            let n = min(cnt, b.remaining());
            b.advance(n);
            cnt -= n;
            if b.remaining() == 0 {
                self.0.pop_front();
            }
        }
        if cnt > 0 {
            panic!("Buf::advance called with cnt - remaining = {} ", cnt);
        }
    }

    fn bytes_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        let mut i = 0;
        let mut len = 0;
        while i < self.0.len() && i < dst.len() {
            let b = self.0[i].bytes();
            dst[i] = IoSlice::new(b);
            len += b.len();
            i += 1;
        }
        len
    }
}

/// RawChannel sends and receives u32 length prefixed messages, which
/// are otherwise just raw bytes.
pub(crate) struct Channel {
    socket: TcpStream,
    header: BytesMut,
    outgoing: BytesDeque,
    incoming: BytesMut,
}

impl Channel {
    pub(crate) fn new(socket: TcpStream) -> Channel {
        Channel {
            socket,
            header: BytesMut::with_capacity(BUF),
            outgoing: BytesDeque(VecDeque::new()),
            incoming: BytesMut::with_capacity(BUF),
        }
    }

    /// Queue an outgoing message. This ONLY queues the message, use
    /// flush to initiate sending. It will fail if the message is
    /// larger then `u32::max_value()`.
    pub(crate) fn queue_send_raw(&mut self, msg: Bytes) -> Result<(), Error> {
        if msg.len() > u32::max_value() as usize {
            return Err(Error::new(
                ErrorKind::InvalidData,
                format!("message too large {} > {}", msg.len(), u32::max_value()),
            ));
        }
        if self.header.remaining_mut() < mem::size_of::<u32>() {
            self.header.reserve(self.header.capacity());
        }
        self.header.put_u32(msg.len() as u32);
        self.outgoing.0.push_back(self.header.split().freeze());
        Ok(self.outgoing.0.push_back(msg))
    }

    /// Same as queue_send_raw, but encodes the message using msgpack
    pub(crate) fn queue_send<T: Serialize>(&mut self, msg: &T) -> Result<(), Error> {     
        self.queue_send_raw(
            mp_encode(msg).map_err(|e| {
                Error::new(ErrorKind::InvalidData, format!("{}", e))
            })?
        )
    }

    /// Initiate sending all outgoing messages and wait for the
    /// process to finish.
    pub(crate) async fn flush(&mut self) -> Result<(), Error> {
        while self.outgoing.0.len() > 0 {
            self.socket.write_buf(&mut self.outgoing).await?;
        }
        Ok(())
    }

    /// Queue one typed message and then flush.
    pub(crate) async fn send_one<T: Serialize>(&mut self, msg: &T) -> Result<(), Error> {
        self.queue_send(msg)?;
        self.flush().await
    }

    async fn fill_buffer(&mut self) -> Result<(), Error> {
        if self.incoming.remaining_mut() < BUF {
            self.incoming.reserve(self.incoming.capacity());
        }
        let n = self.socket.read_buf(&mut self.incoming).await?;
        if n == 0 {
            Err(Error::new(ErrorKind::UnexpectedEof, "end of file"))
        } else {
            Ok(())
        }
    }

    fn decode_from_buffer(&mut self) -> Option<Bytes> {
        if self.incoming.remaining() < mem::size_of::<u32>() {
            None
        } else {
            let len = BigEndian::read_u32(&*self.incoming) as usize;
            if self.incoming.remaining() - mem::size_of::<u32>() < len {
                None
            } else {
                self.incoming.advance(mem::size_of::<u32>());
                Some(self.incoming.split_to(len).freeze())
            }
        }
    }

    /// Receive one message, potentially waiting for one to arrive if
    /// none are presently in the buffer.
    pub(crate) async fn receive_raw(&mut self) -> Result<Bytes, Error> {
        loop {
            match self.decode_from_buffer() {
                Some(msg) => break Ok(msg),
                None => {
                    self.fill_buffer().await?;
                }
            }
        }
    }

    pub(crate) async fn receive<T: DeserializeOwned>(&mut self) -> Result<T, Error> {
        rmp_serde::decode::from_read(&*self.receive_raw().await?)
            .map_err(|e| Error::new(ErrorKind::InvalidData, e))
    }

    /// Receive and decode one or more messages. If any messages fails
    /// to decode, processing will stop and error will be
    /// returned. Some messages may have already been put in the
    /// batch.
    pub(crate) async fn receive_batch<T: DeserializeOwned>(
        &mut self,
        batch: &mut Vec<T>,
    ) -> Result<(), Error> {
        batch.push(self.receive().await?);
        while let Some(b) = self.decode_from_buffer() {
            batch.push(
                rmp_serde::decode::from_read(&*b)
                    .map_err(|e| Error::new(ErrorKind::InvalidData, e))?,
            )
        }
        Ok(())
    }
}