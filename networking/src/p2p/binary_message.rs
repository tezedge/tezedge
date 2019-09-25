// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use bytes::{Buf, BufMut, IntoBuf};
use failure::_core::convert::TryFrom;
use failure::Fail;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crypto::blake2b;
use tezos_encoding::binary_reader::{BinaryReader, BinaryReaderError};
use tezos_encoding::binary_writer::BinaryWriter;
use tezos_encoding::de::from_value as deserialize_from_value;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::hash::Hash;
use tezos_encoding::json_writer::JsonWriter;
use tezos_encoding::ser;

use crate::p2p::binary_message::MessageHashError::SerializationError;

/// Size in bytes of the content length field
pub const CONTENT_LENGTH_FIELD_BYTES: usize = 2;
/// Max allowed message length in bytes
pub const CONTENT_LENGTH_MAX: usize = u16::max_value() as usize;


/// Trait for binary encoding to implement.
///
/// Binary messages could be written by a [`MessageWriter`](super::stream::MessageWriter).
/// To read binary encoding use  [`MessageReader`](super::stream::MessageReader).
pub trait BinaryMessage: Sized {

    /// Produce bytes from the struct.
    fn as_bytes(&self) -> Result<Vec<u8>, ser::Error>;

    /// Create new struct from bytes.
    fn from_bytes(buf: Vec<u8>) -> Result<Self, BinaryReaderError>;
}

impl<T> BinaryMessage for T
    where T: HasEncoding + DeserializeOwned + Serialize + Sized {

    fn as_bytes(&self) -> Result<Vec<u8>, ser::Error> {
        let mut writer = BinaryWriter::new();
        writer.write(self, &Self::encoding())
    }

    fn from_bytes(buf: Vec<u8>) -> Result<Self, BinaryReaderError> {
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
    pub fn from_content(content: &[u8]) -> Result<BinaryChunk, BinaryChunkError> {
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

    /// Get content of the message
    pub fn content(&self) -> &[u8] {
        &self.0[CONTENT_LENGTH_FIELD_BYTES..]
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
        if value.len() < CONTENT_LENGTH_FIELD_BYTES {
            Err(BinaryChunkError::MissingSizeInformation)
        } else if value.len() <= (CONTENT_LENGTH_MAX + CONTENT_LENGTH_FIELD_BYTES) {
            let expected_content_length = value[0..CONTENT_LENGTH_FIELD_BYTES].to_vec().into_buf().get_u16_be() as usize;
            if (expected_content_length + CONTENT_LENGTH_FIELD_BYTES) == value.len() {
                Ok(BinaryChunk(value))
            } else {
                Err(BinaryChunkError::IncorrectSizeInformation { expected: expected_content_length, actual: value.len() })
            }
        } else {
            Err(BinaryChunkError::OverflowError)
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
        Ok(blake2b::digest_256(&bytes))
    }
}

/// Message hash error
#[derive(Debug, Fail)]
pub enum MessageHexableError {
    #[fail(display = "Message to hex serialization error")]
    ToHexSerializationError {
        error: ser::Error
    },
    #[fail(display = "Message from hex serialization error")]
    FromHexSerializationError {
        error: hex::FromHexError
    },
    #[fail(display = "Message from binary serialization error")]
    FromBinarySerializationError {
        error: BinaryReaderError
    },
}

impl From<ser::Error> for MessageHexableError {
    fn from(error: ser::Error) -> Self {
        MessageHexableError::ToHexSerializationError { error }
    }
}

impl From<hex::FromHexError> for MessageHexableError {
    fn from(error: hex::FromHexError) -> Self {
        MessageHexableError::FromHexSerializationError { error }
    }
}

/// Trait for converting messages from/to HEX string
pub trait Hexable: Sized {
    /// Produce HEX string from the struct.
    fn to_hex(&self) -> Result<String, MessageHexableError>;

    /// Create new struct from HEX string.
    fn from_hex(hex: String) -> Result<Self, MessageHexableError>;
}

impl<T: BinaryMessage> Hexable for T {

    fn to_hex(&self) -> Result<String, MessageHexableError> {
        let bytes = self.as_bytes()?;
        Ok(hex::encode(bytes))
    }

    fn from_hex(hex: String) -> Result<Self, MessageHexableError> {
        let bytes = hex::decode(hex)?;
        match Self::from_bytes(bytes) {
            Ok(m) => Ok(m),
            Err(e) => Err(MessageHexableError::FromBinarySerializationError {
                error: e
            })
        }
    }
}