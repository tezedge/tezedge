use std::io;

use bytes::{BufMut, IntoBuf};
use bytes::Buf;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::split::{TcpStreamReadHalf, TcpStreamWriteHalf};
use tokio::net::TcpStream;

use crate::p2p::message::{MESSAGE_LENGTH_FIELD_SIZE, RawBinaryMessage};

/// Holds read and write parts of the message stream.
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

/// Reader of the TCP/IP connection.
pub struct MessageReader {
    /// reader part or the TCP/IP network stream
    stream: TcpStreamReadHalf
}

impl MessageReader {

    /// Read message from network and return message contents in a form of bytes.
    /// Each message is prefixed by a 2 bytes indicating total length of the message.
    pub async fn read_message(&mut self) -> io::Result<RawBinaryMessage> {
        // read encoding length (2 bytes)
        let msg_len_bytes = self.read_message_length_bytes().await?;
        // copy bytes containing encoding length to raw encoding buffer
        let mut all_recv_bytes = vec![];
        all_recv_bytes.extend(&msg_len_bytes);

        // read the message contents
        let msg_len = msg_len_bytes.into_buf().get_u16_be() as usize;
        let mut msg_content_bytes = vec![0u8; msg_len];
        self.stream.read_exact(&mut msg_content_bytes).await?;
        all_recv_bytes.extend(&msg_content_bytes);

        Ok(all_recv_bytes.into())
    }

    /// Read 2 bytes containing total length of the message contents from the network stream.
    /// Total length is encoded as u big endian u16.
    async fn read_message_length_bytes(&mut self) -> io::Result<[u8; MESSAGE_LENGTH_FIELD_SIZE]> {
        let mut msg_len_bytes: [u8; MESSAGE_LENGTH_FIELD_SIZE] = [0; MESSAGE_LENGTH_FIELD_SIZE];
        self.stream.read_exact(&mut msg_len_bytes).await?;
        Ok(msg_len_bytes)
    }
}

pub struct MessageWriter {
    stream: TcpStreamWriteHalf
}

impl MessageWriter {

    /// Construct and write message to network stream.
    ///
    /// # Arguments
    /// * `bytes` - A message contents represented ab bytes
    ///
    /// In case all bytes are successfully written to network stream a raw binary
    /// message is returned as a result.
    pub async fn write_message<'a>(&'a mut self, bytes: &'a [u8]) -> io::Result<RawBinaryMessage> {
        // add length
        let mut msg_with_length = vec![];
        // adds MESSAGE_LENGTH_FIELD_SIZE - 2 bytes with length of encoding
        msg_with_length.put_u16_be(bytes.len() as u16);
        msg_with_length.put(bytes);

        // write serialized encoding bytes
        self.stream.write_all(&msg_with_length).await?;
        Ok(msg_with_length.into())
    }
}