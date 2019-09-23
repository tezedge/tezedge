use std::convert::TryInto;
use std::io;

use bytes::Buf;
use bytes::IntoBuf;
use failure::{Error, Fail};
use log::trace;
use tokio;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::split::{TcpStreamReadHalf, TcpStreamWriteHalf};
use tokio::net::TcpStream;

use crypto::crypto_box::{CryptoError, decrypt, encrypt, PrecomputedKey};
use crypto::nonce::Nonce;
use tezos_encoding::binary_reader::BinaryReaderError;

use crate::p2p::binary_message::{BinaryChunk, BinaryChunkError, BinaryMessage, CONTENT_LENGTH_FIELD_BYTES};
use crate::p2p::peer::PeerId;

/// Max allowed content length in bytes when taking into account extra data added by encryption
pub const CONTENT_LENGTH_MAX: usize = crate::p2p::binary_message::CONTENT_LENGTH_MAX - crypto::crypto_box::BOX_ZERO_BYTES;

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
    #[fail(display = "Message de-serialization error")]
    DeserializationError {
        error: BinaryReaderError
    },
    #[fail(display = "Network error: {}", message)]
    NetworkError {
        message: &'static str,
        error: Error,
    },
}

impl From<tezos_encoding::ser::Error> for StreamError {
    fn from(error: tezos_encoding::ser::Error) -> Self {
        StreamError::SerializationError { error }
    }
}

impl From<std::io::Error> for StreamError {
    fn from(error: std::io::Error) -> Self {
        StreamError::NetworkError { error: error.into(), message: "Stream error" }
    }
}

impl From<BinaryChunkError> for StreamError {
    fn from(error: BinaryChunkError) -> Self {
        StreamError::NetworkError { error: error.into(), message: "Binary chunk error" }
    }
}
impl From<BinaryReaderError> for StreamError {
    fn from(error: BinaryReaderError) -> Self {
        StreamError::DeserializationError { error }
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
    pub async fn read_message(&mut self) -> Result<BinaryChunk, StreamError> {
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

        Ok(all_recv_bytes.try_into()?)
    }

    /// Read 2 bytes containing total length of the message contents from the network stream.
    /// Total length is encoded as u big endian u16.
    async fn read_message_length_bytes(&mut self) -> io::Result<[u8; CONTENT_LENGTH_FIELD_BYTES]> {
        let mut msg_len_bytes: [u8; CONTENT_LENGTH_FIELD_BYTES] = [0; CONTENT_LENGTH_FIELD_BYTES];
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
    pub async fn write_message(&mut self, bytes: &BinaryChunk) -> Result<(), StreamError> {
        Ok(self.stream.write_all(bytes.raw()).await?)
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

        for chunk_content_bytes in message_bytes.chunks(CONTENT_LENGTH_MAX) {
            // encrypt
            let message_bytes_encrypted = match encrypt(chunk_content_bytes, &self.nonce_fetch_increment(), &self.precomputed_key) {
                Ok(msg) => msg,
                Err(error) => return Err(StreamError::FailedToEncryptMessage { error })
            };

            // send
            let chunk = BinaryChunk::from_content(&message_bytes_encrypted)?;
            self.tx.write_message(&chunk).await?;
        }

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

    pub async fn read_message<M>(&mut self) -> Result<M, StreamError>
    where
        M: BinaryMessage
    {
        let mut input_remaining = 0;
        let mut input_data = vec![];

        loop {
            // read
            let message_encrypted = self.rx.read_message().await?;

            // decrypt
            match decrypt(message_encrypted.content(), &self.nonce_fetch_increment(), &self.precomputed_key) {
                Ok(mut message_decrypted) => {
                    trace!("Message received from peer {} as hex: \n{}", self.peer_id, hex::encode(&message_decrypted));
                    if input_remaining >= message_decrypted.len() {
                        input_remaining -= message_decrypted.len();
                    } else {
                        input_remaining = 0;
                    }

                    input_data.append(&mut message_decrypted);

                    if input_remaining == 0 {
                        match M::from_bytes(input_data.clone()) {
                            Ok(message) => break Ok(message),
                            Err(BinaryReaderError::Underflow { bytes }) => input_remaining += bytes,
                            Err(e) => break Err(e.into()),
                        }
                    }
                }
                Err(error) => {
                    break Err(StreamError::FailedToDecryptMessage { error })
                }
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