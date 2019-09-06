use std::io;

use bytes::{BufMut, IntoBuf};
use bytes::Buf;
use failure::Fail;
use log::trace;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::split::{TcpStreamReadHalf, TcpStreamWriteHalf};
use tokio::net::TcpStream;

use crypto::crypto_box::{encrypt, decrypt, PrecomputedKey, CryptoError};
use crypto::nonce::Nonce;

use crate::p2p::binary_message::{BinaryMessage, MESSAGE_LENGTH_FIELD_SIZE, RawBinaryMessage};
use crate::p2p::peer::PeerId;

/// This is common error that might happen when communicating with peer over the network.
#[derive(Debug, Fail)]
pub enum StreamError {
    #[fail(display = "Failed to encrypt message")]
    FailedToEncryptMessage {
        error: CryptoError
    },
    #[fail(display = "Failed to decrypt message")]
    FailedToDecryptMessage {
        error: CryptoError
    },
    #[fail(display = "Message serialization error")]
    SerializationError {
        error: tezos_encoding::ser::Error
    },
    #[fail(display = "Network error: {}", message)]
    NetworkError {
        message: &'static str,
        error: io::Error,
    },
}

impl From<tezos_encoding::ser::Error> for StreamError {
    fn from(error: tezos_encoding::ser::Error) -> Self {
        StreamError::SerializationError { error }
    }
}

impl From<std::io::Error> for StreamError {
    fn from(error: std::io::Error) -> Self {
        StreamError::NetworkError { error, message: "Network error" }
    }
}

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
    pub async fn read_message(&mut self) -> Result<RawBinaryMessage, StreamError> {
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
    pub async fn write_message<'a>(&'a mut self, bytes: &'a [u8]) -> Result<RawBinaryMessage, StreamError> {
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

/// The `EncryptedMessageWriter` encapsulates process of the encrypted outgoing message transmission.
/// This process involves (not only) nonce increment, encryption and network transmission.
pub struct EncryptedMessageWriter {
    /// Precomputed key is created from merge of peer public key and our secret key.
    /// It's used to speedup of crypto operations.
    precomputed_key: PrecomputedKey,
    /// Nonce used to encrypt outgoing messages
    nonce_local: Nonce,
    /// Outgoing message writer
    tx: MessageWriter,
    /// Peer ID is created as hex string representation of peer public key bytes.
    peer_id: PeerId,
}

impl EncryptedMessageWriter {

    pub fn new(tx: MessageWriter, precomputed_key: PrecomputedKey, nonce_local: Nonce, peer_id: PeerId) -> Self {
        EncryptedMessageWriter { tx, precomputed_key, nonce_local, peer_id }
    }

    pub async fn write_message<'a>(&'a mut self, message: &'a impl BinaryMessage) -> Result<(), StreamError> {
        let message_bytes = message.as_bytes()?;
        trace!("Message to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_bytes));

        // encrypt
        let message_encrypted = match encrypt(&message_bytes, &self.nonce_fetch_increment(), &self.precomputed_key) {
            Ok(msg) => msg,
            Err(error) => return Err(StreamError::FailedToEncryptMessage { error })
        };
        trace!("Message (enc) to send to peer {} as hex (without length): \n{}", self.peer_id, hex::encode(&message_encrypted));

        // send
        self.tx.write_message(&message_encrypted).await?;

        Ok(())
    }

    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.nonce_local.increment();
        std::mem::replace(&mut self.nonce_local, incremented)
    }
}

/// The `MessageReceiver` encapsulates process of the encrypted incoming message transmission.
/// This process involves (not only) nonce increment, encryption and network transmission.
pub struct EncryptedMessageReader {
    /// Precomputed key is created from merge of peer public key and our secret key.
    /// It's used to speedup of crypto operations.
    precomputed_key: PrecomputedKey,
    /// Nonce used to decrypt received messages
    nonce_remote: Nonce,
    /// Incoming message reader
    rx: MessageReader,
    /// Peer ID is created as hex string representation of peer public key bytes.
    peer_id: PeerId,
}

impl EncryptedMessageReader {

    pub fn new(rx: MessageReader, precomputed_key: PrecomputedKey, nonce_remote: Nonce, peer_id: PeerId) -> Self {
        EncryptedMessageReader { rx, precomputed_key, nonce_remote, peer_id }
    }

    pub async fn read_message(&mut self) -> Result<Vec<u8>, StreamError> {
        // read
        let message_encrypted = self.rx.read_message().await?;

        // decrypt
        match decrypt(message_encrypted.get_contents(), &self.nonce_fetch_increment(), &self.precomputed_key) {
            Ok(message) => {
                trace!("Message received from peer {} as hex: \n{}", self.peer_id, hex::encode(&message));
                Ok(message)
            }
            Err(error) => {
                Err(StreamError::FailedToDecryptMessage { error })
            }
        }
    }

    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.nonce_remote.increment();
        std::mem::replace(&mut self.nonce_remote, incremented)
    }
}