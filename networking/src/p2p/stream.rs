// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module encapsulates p2p communication between peers.
//!
//! It provides message packaging from/to binary format, encryption, message nonce handling.

use std::{convert::TryInto, io, pin::Pin, task::Poll};

use bytes::Buf;
use failure::_core::time::Duration;
use failure::{Error, Fail};
use slog::{debug, trace, FnValue, Logger};
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadBuf, ReadHalf, WriteHalf};
use tokio::net::TcpStream;

use crypto::crypto_box::PrecomputedKey;
use crypto::nonce::Nonce;
use crypto::CryptoError;
use tezos_encoding::{
    binary_async_reader::BinaryRead,
    binary_reader::{BinaryReaderError, BinaryReaderErrorKind},
    binary_writer::BinaryWriterError,
};
use tezos_messages::p2p::binary_message::{
    BinaryChunk, BinaryChunkError, BinaryMessage, CONTENT_LENGTH_FIELD_BYTES,
};

/// Max allowed content length in bytes when taking into account extra data added by encryption
pub const CONTENT_LENGTH_MAX: usize =
    tezos_messages::p2p::binary_message::CONTENT_LENGTH_MAX - crypto::crypto_box::BOX_ZERO_BYTES;

/// This is common error that might happen when communicating with peer over the network.
#[derive(Debug, Fail)]
pub enum StreamError {
    #[fail(display = "Failed to encrypt message")]
    FailedToEncryptMessage { error: CryptoError },
    #[fail(display = "Failed to decrypt message")]
    FailedToDecryptMessage { error: CryptoError },
    #[fail(display = "Message serialization error: {}", error)]
    SerializationError { error: BinaryWriterError },
    #[fail(display = "Message de-serialization error: {}", error)]
    DeserializationError { error: BinaryReaderError },
    #[fail(display = "Network error: {}, cause: {}", message, error)]
    NetworkError { message: &'static str, error: Error },
}

impl From<BinaryWriterError> for StreamError {
    fn from(error: BinaryWriterError) -> Self {
        StreamError::SerializationError { error }
    }
}

impl From<std::io::Error> for StreamError {
    fn from(error: std::io::Error) -> Self {
        StreamError::NetworkError {
            error: error.into(),
            message: "Stream error",
        }
    }
}

impl From<BinaryChunkError> for StreamError {
    fn from(error: BinaryChunkError) -> Self {
        StreamError::NetworkError {
            error: error.into(),
            message: "Binary chunk error",
        }
    }
}

impl From<BinaryReaderError> for StreamError {
    fn from(error: BinaryReaderError) -> Self {
        StreamError::DeserializationError { error }
    }
}

impl slog::Value for StreamError {
    fn serialize(
        &self,
        _record: &slog::Record,
        key: slog::Key,
        serializer: &mut dyn slog::Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self))
    }
}

/// Holds read and write parts of the message stream.
pub struct MessageStream {
    reader: ReadHalf<TcpStream>,
    writer: MessageWriter,
}

impl MessageStream {
    fn new(stream: TcpStream) -> MessageStream {
        let _ = stream.set_linger(Some(Duration::from_secs(2)));
        let _ = stream.set_nodelay(true);

        let (rx, tx) = tokio::io::split(stream);
        MessageStream {
            reader: rx,
            writer: MessageWriter { stream: tx },
        }
    }

    #[inline]
    pub fn split(self) -> (ReadHalf<TcpStream>, MessageWriter) {
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
    stream: ReadHalf<TcpStream>,
}

impl MessageReader {
    /// Read message from network and return message contents in a form of bytes.
    /// Each message is prefixed by a 2 bytes indicating total length of the message.
    pub async fn read_message(&mut self) -> Result<BinaryChunk, StreamError> {
        // read encoding length (2 bytes)
        let msg_len_bytes = self.read_message_length_bytes().await?;
        // copy bytes containing encoding length to`` raw encoding buffer
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
    stream: WriteHalf<TcpStream>,
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
    pub fn new(
        tx: MessageWriter,
        precomputed_key: PrecomputedKey,
        nonce_local: Nonce,
        log: Logger,
    ) -> Self {
        EncryptedMessageWriter {
            tx,
            precomputed_key,
            nonce_local,
            log,
        }
    }

    pub async fn write_message<'a>(
        &'a mut self,
        message: &'a impl BinaryMessage,
    ) -> Result<(), StreamError> {
        let message_bytes = message.as_bytes()?;
        trace!(self.log, "Writing message"; "message" => FnValue(|_| hex::encode(&message_bytes)));

        for chunk_content_bytes in message_bytes.chunks(CONTENT_LENGTH_MAX) {
            // encrypt
            let nonce = self.nonce_fetch_increment();
            let message_bytes_encrypted =
                match self.precomputed_key.encrypt(chunk_content_bytes, &nonce) {
                    Ok(msg) => msg,
                    Err(error) => return Err(StreamError::FailedToEncryptMessage { error }),
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
    /// Nonce used to decrypt received messages
    crypt_data: Option<CryptData>,
    /// Incoming message reader
    read: ReadHalf<TcpStream>,
    /// Async read state
    state: ReadState,
    /// Logger
    log: Logger,
}

impl EncryptedMessageReader {
    /// Create new encrypted message from async reader and peer data
    pub fn new(read: ReadHalf<TcpStream>, log: Logger) -> Self {
        EncryptedMessageReader {
            read,
            crypt_data: None,
            state: ReadState::ReadSize {
                mode: ReadMode::Continious,
                buff: [0; 2],
                offset: 0,
            },
            log,
        }
    }

    /// Read message from network and return message contents in a form of bytes.
    /// Each message is prefixed by a 2 bytes indicating total length of the message.
    pub async fn read_connection_message(&mut self) -> Result<BinaryChunk, StreamError> {
        // read encoding length (2 bytes)
        let msg_len_bytes = self.read_message_length_bytes().await?;
        // copy bytes containing encoding length to`` raw encoding buffer
        let mut all_recv_bytes = vec![];
        all_recv_bytes.extend(&msg_len_bytes);

        // read the message contents
        let msg_len = (&msg_len_bytes[..]).get_u16() as usize;
        let mut msg_content_bytes = vec![0u8; msg_len];
        self.read.read_exact(&mut msg_content_bytes).await?;
        all_recv_bytes.extend(&msg_content_bytes);

        Ok(all_recv_bytes.try_into()?)
    }

    /// Read 2 bytes containing total length of the message contents from the network stream.
    /// Total length is encoded as u big endian u16.
    async fn read_message_length_bytes(&mut self) -> io::Result<[u8; CONTENT_LENGTH_FIELD_BYTES]> {
        let mut msg_len_bytes: [u8; CONTENT_LENGTH_FIELD_BYTES] = [0; CONTENT_LENGTH_FIELD_BYTES];
        self.read.read_exact(&mut msg_len_bytes).await?;
        Ok(msg_len_bytes)
    }

    pub fn set_crypt_data(&mut self, precomputed_key: PrecomputedKey, nonce_remote: Nonce) {
        self.crypt_data = Some(CryptData {
            precomputed_key,
            nonce_remote,
        });
    }

    /// Consume content of inner message reader into specific message
    pub async fn read_dynamic_message<M>(&mut self) -> Result<M, StreamError>
    where
        M: BinaryMessage,
    {
        self.state = ReadState::ReadSize {
            mode: ReadMode::Continious,
            buff: [0; 2],
            offset: 0,
        };
        let message = M::read_dynamic(self).await?;
        Ok(message)
    }

    /// Consume content of a single chunk and decode it into a message of specified type
    pub async fn read_message<M>(&mut self) -> Result<M, StreamError>
    where
        M: BinaryMessage,
    {
        self.state = ReadState::ReadSize {
            mode: ReadMode::Chunked,
            buff: [0; 2],
            offset: 0,
        };
        let message = M::read(self).await?;
        Ok(message)
    }

    pub fn unsplit(self, tx: EncryptedMessageWriter) -> TcpStream {
        self.read.unsplit(tx.tx.stream)
    }

    #[inline]
    fn int_overflow_error() -> io::Error {
        io::Error::new(io::ErrorKind::Other, "integer operation overflow")
    }

    #[inline]
    fn increment_offset(offset: usize, old_rem: usize, rem: usize) -> io::Result<usize> {
        Self::checked_add(offset, Self::checked_sub(old_rem, rem)?)
    }

    #[inline]
    fn checked_sub(a: usize, b: usize) -> io::Result<usize> {
        Ok(a.checked_sub(b).ok_or_else(&Self::int_overflow_error)?)
    }

    #[inline]
    fn checked_add(a: usize, b: usize) -> io::Result<usize> {
        Ok(a.checked_add(b).ok_or_else(&Self::int_overflow_error)?)
    }
}

/// Chunk reading mode
#[derive(Debug, Clone, Copy)]
enum ReadMode {
    /// Message is limited by the size of the chunk
    Chunked,
    /// Message may consist of several chunks
    Continious,
}

#[derive(Debug, Clone)]
enum ReadState {
    ReadSize {
        mode: ReadMode,
        buff: [u8; 2],
        offset: usize,
    },
    ReadData {
        mode: ReadMode,
        buff: Vec<u8>,
        offset: usize,
    },
    DataAvail {
        mode: ReadMode,
        buff: Vec<u8>,
        offset: usize,
    },
}

struct CryptData {
    precomputed_key: PrecomputedKey,
    nonce_remote: Nonce,
}

impl CryptData {
    fn fetch_increment_nonce(&mut self) -> Nonce {
        let nonce = self.nonce_remote.increment();
        std::mem::replace(&mut self.nonce_remote, nonce)
    }
}

use tokio::io::AsyncRead;

impl AsyncRead for EncryptedMessageReader {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> std::task::Poll<io::Result<()>> {
        let me = &mut *self;
        loop {
            match me.state {
                ReadState::ReadSize {
                    mode,
                    mut buff,
                    offset,
                } => {
                    let mut rbuff = ReadBuf::new(&mut buff[offset..]);
                    let rem = rbuff.remaining();
                    match Pin::new(&mut me.read).poll_read(cx, &mut rbuff) {
                        Poll::Ready(res) => res?,
                        Poll::Pending => return Poll::Pending,
                    }
                    let rem_new = rbuff.remaining();
                    if rem == rem_new {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "eof reading chunk size",
                        ))
                        .into();
                    }
                    me.state = if rem_new == 0 {
                        let size: usize = u16::from_be_bytes(buff) as usize;
                        debug!(me.log, "Chunk size is ready"; "size" => size);
                        ReadState::ReadData {
                            mode,
                            buff: vec![0; size],
                            offset: 0,
                        }
                    } else {
                        ReadState::ReadSize {
                            mode,
                            buff,
                            offset: Self::increment_offset(offset, rem, rem_new)?,
                        }
                    };
                }
                ReadState::ReadData {
                    mode,
                    ref mut buff,
                    offset,
                } => {
                    let mut rbuff = ReadBuf::new(&mut buff.as_mut_slice()[offset..]);
                    let rem = rbuff.remaining();
                    match Pin::new(&mut me.read).poll_read(cx, &mut rbuff) {
                        Poll::Ready(res) => res?,
                        Poll::Pending => return Poll::Pending,
                    }
                    let rem_new = rbuff.remaining();
                    if rem == rem_new {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "eof reading chunk data",
                        ))
                        .into();
                    }
                    me.state = if rem_new == 0 {
                        if let Some(ref mut crypt_data) = me.crypt_data {
                            debug!(me.log, "Encrypted chunk is ready"; "size" => buff.len());
                            let nonce = crypt_data.fetch_increment_nonce();
                            match crypt_data.precomputed_key.decrypt(buff.as_slice(), &nonce) {
                                Ok(message_decrypted) => {
                                    trace!(me.log, "Message received"; "message" => FnValue(|_| hex::encode(&message_decrypted)));
                                    ReadState::DataAvail {
                                        mode,
                                        buff: message_decrypted,
                                        offset: 0,
                                    }
                                }
                                Err(error) => {
                                    return Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        format!("error decryping a chunk: {}", error),
                                    ))
                                    .into();
                                }
                            }
                        } else {
                            debug!(me.log, "Non-encrypted chunk is ready"; "size" => buff.len());
                            ReadState::DataAvail {
                                mode,
                                buff: std::mem::replace(buff, vec![]),
                                offset: 0,
                            }
                        }
                    } else {
                        ReadState::ReadData {
                            mode,
                            buff: std::mem::replace(buff, vec![]),
                            offset: Self::increment_offset(offset, rem, rem_new)?,
                        }
                    };
                }
                // requested more data then available in the chunk in chunked mode
                ReadState::DataAvail {
                    mode,
                    ref mut buff,
                    offset,
                } if Self::checked_sub(buff.len(), offset)? < buf.remaining() => {
                    if let ReadMode::Chunked = mode {
                        return Err(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "end of chunk reading one-chunk message",
                        ))
                        .into();
                    } else {
                        // we should continue to the next chunk
                        debug!(me.log, "Next chunk is needed");
                        me.state = ReadState::ReadSize {
                            mode,
                            buff: [0; 2],
                            offset: 0,
                        }
                    }
                }
                ReadState::DataAvail {
                    mode,
                    ref mut buff,
                    offset,
                } => {
                    let avail = Self::checked_sub(buff.len(), offset)?;
                    let amt = std::cmp::min(avail, buf.remaining());
                    trace!(me.log, "Data requested"; "requested size" => buf.remaining(), "available bytes" => avail);
                    let (a, _) = buff.as_slice()[offset..].split_at(amt);
                    buf.put_slice(a);
                    trace!(me.log, "Data provided"; "data" => FnValue(|_| hex::encode(a)));
                    me.state = ReadState::DataAvail {
                        mode,
                        buff: std::mem::replace(buff, vec![]),
                        offset: Self::checked_add(offset, amt)?,
                    };
                    if buf.remaining() == 0 {
                        trace!(me.log, "Data is ready");
                        return Poll::Ready(Ok(()));
                    }
                }
            }
        }
    }
}

impl BinaryRead for EncryptedMessageReader {
    fn remaining(&self) -> Result<usize, BinaryReaderError> {
        match self.state {
            ReadState::ReadSize { .. }
            | ReadState::ReadData {
                mode: ReadMode::Continious,
                ..
            }
            | ReadState::DataAvail {
                mode: ReadMode::Continious,
                ..
            } => Err(BinaryReaderErrorKind::RemainingUnknown)?,
            ReadState::ReadData {
                mode: _,
                ref buff,
                offset,
            }
            | ReadState::DataAvail {
                mode: _,
                ref buff,
                offset,
            } => Ok(Self::checked_sub(buff.len(), offset)?),
        }
    }
}
