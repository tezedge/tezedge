// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module encapsulates p2p communication between peers.
//!
//! It provides message packaging from/to binary format, encryption, message nonce handling.

use std::convert::TryInto;
use std::io;

use bytes::Buf;
use failure::{Error, Fail};
use failure::_core::time::Duration;
use slog::{FnValue, Logger, trace};
use tokio::io::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::prelude::*;

use crypto::CryptoError;
use crypto::crypto_box::PrecomputedKey;
use crypto::nonce::Nonce;
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError, BinaryMessage, CONTENT_LENGTH_FIELD_BYTES};

/// Max allowed content length in bytes when taking into account extra data added by encryption
pub const CONTENT_LENGTH_MAX: usize = tezos_messages::p2p::binary_message::CONTENT_LENGTH_MAX - crypto::crypto_box::BOX_ZERO_BYTES;

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
    #[fail(display = "Message de-serialization error: {:?}", error)]
    DeserializationError {
        error: BinaryReaderError
    },
    #[fail(display = "Network error: {}, cause: {}", message, error)]
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

impl slog::Value for StreamError {
    fn serialize(&self, _record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}


/// Holds read and write parts of the message stream.
pub struct MessageStream {
    reader: MessageReader,
    writer: MessageWriter,
}

impl MessageStream {
    fn new(stream: TcpStream) -> MessageStream {
        let _ = stream.set_linger(Some(Duration::from_secs(2)));
        let _ = stream.set_nodelay(true);

        let (rx, tx) = tokio::io::split(stream);
        MessageStream {
            reader: MessageReader { stream: rx },
            writer: MessageWriter { stream: tx },
        }
    }

    #[inline]
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
    stream: ReadHalf<TcpStream>
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
        let msg_len = (&msg_len_bytes[..]).get_u16() as usize;
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
    stream: WriteHalf<TcpStream>
}

impl MessageWriter {
    /// Construct and write message to network stream.
    ///
    /// # Arguments
    /// * `bytes` - A message contents represented ab bytes
    ///
    /// In case all bytes are successfully written to network stream a raw binary
    /// message is returned as a result.
    #[inline]
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
    /// Logger
    log: Logger,
}

impl EncryptedMessageWriter {
    pub fn new(tx: MessageWriter, precomputed_key: PrecomputedKey, nonce_local: Nonce, log: Logger) -> Self {
        EncryptedMessageWriter { tx, precomputed_key, nonce_local, log }
    }

    pub async fn write_message<'a>(&'a mut self, message: &'a impl BinaryMessage) -> Result<(), StreamError> {
        let message_bytes = message.as_bytes()?;
        trace!(self.log, "Writing message"; "message" => FnValue(|_| hex::encode(&message_bytes)));

        for chunk_content_bytes in message_bytes.chunks(CONTENT_LENGTH_MAX) {
            // encrypt
            let nonce = self.nonce_fetch_increment();
            let message_bytes_encrypted = match self.precomputed_key.encrypt(chunk_content_bytes, &nonce) {
                Ok(msg) => msg,
                Err(error) => return Err(StreamError::FailedToEncryptMessage { error })
            };

            // send
            let chunk = BinaryChunk::from_content(&message_bytes_encrypted)?;
            self.tx.write_message(&chunk).await?;
        }

        Ok(())
    }

    #[inline]
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
    /// Logger
    log: Logger,
}

impl EncryptedMessageReader {
    /// Create new encrypted message from async reader and peer data
    pub fn new(rx: MessageReader, precomputed_key: PrecomputedKey, nonce_remote: Nonce, log: Logger) -> Self {
        EncryptedMessageReader { rx, precomputed_key, nonce_remote, log }
    }

    /// Consume content of inner message reader into specific message
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
            let nonce = self.nonce_fetch_increment();
            match self.precomputed_key.decrypt(message_encrypted.content(), &nonce) {
                Ok(mut message_decrypted) => {
                    trace!(self.log, "Message received"; "message" => FnValue(|_| hex::encode(&message_decrypted)));
                    if input_remaining >= message_decrypted.len() {
                        input_remaining -= message_decrypted.len();
                    } else {
                        input_remaining = 0;
                    }

                    input_data.append(&mut message_decrypted);

                    if input_remaining == 0 {
                        match M::from_bytes(&input_data) {
                            Ok(message) => break Ok(message),
                            Err(BinaryReaderError::Underflow { bytes }) => input_remaining += bytes,
                            Err(e) => break Err(e.into()),
                        }
                    }
                }
                Err(error) => {
                    break Err(StreamError::FailedToDecryptMessage { error });
                }
            }
        }
    }

    #[inline]
    fn nonce_fetch_increment(&mut self) -> Nonce {
        let incremented = self.nonce_remote.increment();
        std::mem::replace(&mut self.nonce_remote, incremented)
    }

    pub fn unsplit(self, tx: EncryptedMessageWriter) -> TcpStream {
        self.rx.stream.unsplit(tx.tx.stream)
    }
}