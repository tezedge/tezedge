use bytes::{Buf, IntoBuf, BufMut};
use failure::_core::convert::TryFrom;
use failure::Fail;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crypto::blake2b;
use tezos_encoding::binary_reader::BinaryReader;
use tezos_encoding::binary_writer::BinaryWriter;
use tezos_encoding::de;
use tezos_encoding::de::from_value as deserialize_from_value;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::hash::Hash;
use tezos_encoding::json_writer::JsonWriter;
use tezos_encoding::ser;

use crate::p2p::binary_message::MessageHashError::SerializationError;

pub const MESSAGE_LENGTH_FIELD_SIZE: usize = 2;
pub const CONTENT_LENGTH_MAX: usize =  0xFFFF - 0x0F;

/// Trait for binary encoding to implement.
///
/// Binary messages could be written by a [`MessageWriter`](super::stream::MessageWriter).
/// To read binary encoding use  [`MessageReader`](super::stream::MessageReader).
pub trait BinaryMessage: Sized {

    /// Produce bytes from the struct.
    fn as_bytes(&self) -> Result<Vec<u8>, ser::Error>;

    /// Create new struct from bytes.
    fn from_bytes(buf: Vec<u8>) -> Result<Self, de::Error>;
}

impl<T> BinaryMessage for T
    where T: HasEncoding + DeserializeOwned + Serialize + Sized {

    fn as_bytes(&self) -> Result<Vec<u8>, ser::Error> {
        let mut writer = BinaryWriter::new();
        writer.write(self, &Self::encoding())
    }

    /// Create new struct from bytes.
    fn from_bytes(buf: Vec<u8>) -> Result<Self, de::Error> {
        let reader = BinaryReader::new();
        let value = reader.read(buf, &Self::encoding())?;
        deserialize_from_value(&value)
    }
}

/// Represents binary raw encoding received from peer node.
///
/// Difference from [`BinaryMessage`] is that it also contains [`MESSAGE_LENGTH_FIELD_SIZE`] bytes
/// of information about how many bytes is the actual encoding.
pub struct BinaryChunk(Vec<u8>);

impl BinaryChunk {

    /// Create new `BinaryChunk` from input content.
    pub fn from_bytes(content: impl ExactSizeIterator<Item=u8>) -> Result<BinaryChunk, BinaryChunkError> {
        if content.len() <= CONTENT_LENGTH_MAX {
            // add length
            let mut bytes = vec![];
            // adds MESSAGE_LENGTH_FIELD_SIZE -- 2 bytes with length of the content
            bytes.put_u16_be(content.len() as u16);
            // append data
            bytes.extend(content);

            Ok(BinaryChunk(bytes))
        } else {
            Err(BinaryChunkError::OverflowError)
        }
    }

    /// Gets raw data (including encoded content size)
    pub fn raw(&self) -> &Vec<u8> {
        &self.0
    }

    /// Gets only contents data
    pub fn contents(&self) -> &[u8] {
        &self.0[MESSAGE_LENGTH_FIELD_SIZE..]
    }
}

/// `BinaryChunk` error
#[derive(Debug, Fail)]
pub enum BinaryChunkError {
    #[fail(display = "Overflow error")]
    OverflowError,
    #[fail(display = "Missing size information")]
    MissingSizeInformation,
    #[fail(display = "Incorrect content size information. expected={}, actual={}", expected, actual)]
    IncorrectSizeInformation {
        expected: usize,
        actual: usize
    },
}

/// Convert `Vec<u8>` into `BinaryChunk`. It is required that input `Vec<u8>`
/// contains also information about the content length in its first 2 bytes.
impl TryFrom<Vec<u8>> for BinaryChunk {
    type Error = BinaryChunkError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() < MESSAGE_LENGTH_FIELD_SIZE {
            Err(BinaryChunkError::MissingSizeInformation)
        } else if value.len() <= (CONTENT_LENGTH_MAX + MESSAGE_LENGTH_FIELD_SIZE) {
            let expected_length = value[0..MESSAGE_LENGTH_FIELD_SIZE].to_vec().into_buf().get_u16_be() as usize;
            if (expected_length + MESSAGE_LENGTH_FIELD_SIZE) == value.len() {
                Ok(BinaryChunk(value))
            } else {
                Err(BinaryChunkError::IncorrectSizeInformation { expected: expected_length, actual: value.len() })
            }
        } else {
            Err(BinaryChunkError::OverflowError)
        }
    }
}

/// Produces list of [`BinaryChunk`] from [`Vec<u8>`].
pub struct BinaryChunkProducer(Vec<u8>);

impl From<Vec<u8>> for BinaryChunkProducer {
    fn from(data: Vec<u8>) -> Self {
        BinaryChunkProducer(data)
    }
}

impl Iterator for BinaryChunkProducer {
    type Item = BinaryChunk;

    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            None
        } else if self.0.len() < CONTENT_LENGTH_MAX {
            Some(BinaryChunk::from_bytes(self.0.drain(0..)).expect("Failed to create binary chunk"))
        } else {
            Some(BinaryChunk::from_bytes(self.0.drain(0..CONTENT_LENGTH_MAX)).expect("Failed to create binary chunk"))
        }
    }
}

/// Trait for json encoding to implement.
pub trait JsonMessage {

    /// Produce JSON from the struct.
    fn as_json(&self) -> Result<String, ser::Error>;
}

impl<T> JsonMessage for T
    where T: HasEncoding + Serialize + Sized {

    fn as_json(&self) -> Result<String, ser::Error> {
        let mut writer = JsonWriter::new();
        writer.write(self, &Self::encoding())
    }
}

/// Message hash error
#[derive(Debug, Fail)]
pub enum MessageHashError {
    #[fail(display = "Message serialization error")]
    SerializationError {
        error: ser::Error
    },
}

impl From<ser::Error> for MessageHashError {
    fn from(error: ser::Error) -> Self {
        SerializationError { error }
    }
}

/// Trait for getting hash of the message.
pub trait MessageHash {
    fn message_hash(&self) -> Result<Hash, MessageHashError>;
}

impl<T: BinaryMessage> MessageHash for T {
    fn message_hash(&self) -> Result<Hash, MessageHashError> {
        let bytes = self.as_bytes()?;
        Ok(blake2b::digest(&bytes))
    }
}