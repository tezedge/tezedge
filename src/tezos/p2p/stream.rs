use bytes::Buf;
use failure::Error;
use futures::io::{WriteHalf, ReadHalf};
use futures::prelude::*;
use bytes::{BufMut, IntoBuf};
use romio::TcpStream;

use super::message::{RawBinaryMessage, MESSAGE_LENGTH_FIELD_SIZE};

pub struct MessageStream {
    reader: MessageReader,
    writer: MessageWriter,
}

impl MessageStream {
    fn new(stream: TcpStream) -> MessageStream {
        let (rx, tx) = stream.split();
        MessageStream {
            reader: MessageReader { stream: rx },
            writer: MessageWriter { stream: tx }
        }
    }

    pub fn split(self) -> (MessageReader, MessageWriter) {
        (self.reader, self.writer)
    }
}

impl From<TcpStream> for MessageStream {
    fn from(stream: TcpStream) -> Self {
        MessageStream::new(stream)
    }
}

pub struct MessageReader {
    stream: ReadHalf<TcpStream>
}

impl MessageReader {

    pub async fn read_message(&mut self) -> Result<RawBinaryMessage, Error> {
        // read message length (2 bytes)
        let msg_len_bytes = await!(self.read_message_length_bytes())?;
        // copy bytes containing message length to raw message buffer
        let mut all_recv_bytes = vec![];
        all_recv_bytes.extend_from_slice(&msg_len_bytes);

        // read the message content
        let msg_len = msg_len_bytes.into_buf().get_u16_be() as usize;
        let mut msg_content_bytes =vec![0u8; msg_len];
        await!(self.stream.read_exact(&mut msg_content_bytes))?;
        all_recv_bytes.extend_from_slice(&msg_content_bytes);

        Ok(all_recv_bytes.into())
    }

    async fn read_message_length_bytes(&mut self) -> Result<Vec<u8>, Error> {
        let mut msg_len_bytes = vec![0u8; MESSAGE_LENGTH_FIELD_SIZE];
        await!(self.stream.read_exact(&mut msg_len_bytes))?;
        Ok(msg_len_bytes)
    }
}

pub struct MessageWriter {
    stream: WriteHalf<TcpStream>
}

impl MessageWriter {

    pub async fn write_message<'a>(&'a mut self, bytes: &'a [u8]) -> Result<RawBinaryMessage, Error> {
        // add length
        let mut msg_with_length = vec![];
        // adds MESSAGE_LENGTH_FIELD_SIZE - 2 bytes with length of message
        msg_with_length.put_u16_be(bytes.len() as u16);
        msg_with_length.put(bytes);

        // write serialized message bytes
        await!(self.stream.write_all(&msg_with_length))?;
        await!(self.stream.flush())?;
        Ok(msg_with_length.into())
    }
}