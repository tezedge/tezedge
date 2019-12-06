// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bytes::{Buf, BufMut, IntoBuf};
use failure::_core::convert::TryFrom;
use failure::Fail;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crypto::blake2b;
use crypto::hash::Hash;
use tezos_encoding::binary_reader::{BinaryReader, BinaryReaderError};
use tezos_encoding::binary_writer::BinaryWriter;
use tezos_encoding::de::from_value as deserialize_from_value;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::json_writer::JsonWriter;
use tezos_encoding::ser;

use crate::p2p::binary_message::MessageHashError::SerializationError;

/// Size in bytes of the content length field
pub const CONTENT_LENGTH_FIELD_BYTES: usize = 2;
/// Max allowed message length in bytes
pub const CONTENT_LENGTH_MAX: usize = u16::max_value() as usize;

pub mod cache {
    use std::fmt;

    use serde::{Deserialize, Deserializer};

    pub trait CacheReader {
        fn get(&self) -> Option<Vec<u8>>;
    }

    pub trait CacheWriter {
        fn put(&mut self, body: &[u8]);
    }

    pub trait CachedData {
        fn cache_reader(&self) -> &dyn CacheReader;
        fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter>;
    }

    #[derive(Clone, Default)]
    pub struct BinaryDataCache {
        data: Option<Vec<u8>>
    }

    impl CacheReader for BinaryDataCache {
        #[inline]
        fn get(&self) -> Option<Vec<u8>> {
            self.data.as_ref().cloned()
        }
    }

    impl CacheWriter for BinaryDataCache {
        #[inline]
        fn put(&mut self, body: &[u8]) {
            self.data.replace(body.to_vec());
        }
    }

    impl PartialEq for BinaryDataCache {
        #[inline]
        fn eq(&self, _: &Self) -> bool {
            true
        }
    }

    impl Eq for BinaryDataCache {}

    impl<'de> Deserialize<'de> for BinaryDataCache {
        fn deserialize<D>(_: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>
        {
            Ok(BinaryDataCache::default())
        }
    }

    impl fmt::Debug for BinaryDataCache {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.get() {
                Some(data) => write!(f, "BinaryDataCache {{ has_value: true, len: {} }}", data.len()),
                None => write!(f, "BinaryDataCache {{ has_value: false }}"),
            }
        }
    }

    #[derive(Clone, PartialEq, Default)]
    pub struct NeverCache;

    impl<'de> Deserialize<'de> for NeverCache {
        fn deserialize<D>(_: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>
        {
            Ok(NeverCache::default())
        }
    }

    impl CacheReader for NeverCache {
        #[inline]
        fn get(&self) -> Option<Vec<u8>> {
            None
        }
    }

    impl CacheWriter for NeverCache {
        #[inline]
        fn put(&mut self, _: &[u8]) {
            // ..
        }
    }

    impl fmt::Debug for NeverCache {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "NeverCache {{ }}")
        }
    }

}

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
    where T: HasEncoding + cache::CachedData + DeserializeOwned + Serialize + Sized {

    #[inline]
    fn as_bytes(&self) -> Result<Vec<u8>, ser::Error> {
        match self.cache_reader().get() {
            Some(data) => Ok(data),
            None => BinaryWriter::new().write(self, &Self::encoding())
        }
    }

    #[inline]
    fn from_bytes(buf: Vec<u8>) -> Result<Self, BinaryReaderError> {
        let body = buf.clone();
        let value = BinaryReader::new().read(buf, &Self::encoding())?;
        let mut myself: Self = deserialize_from_value(&value)?;
        if let Some(cache_writer) = myself.cache_writer() {
            cache_writer.put(&body);
        }
        Ok(myself)
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
    #[inline]
    pub fn raw(&self) -> &Vec<u8> {
        &self.0
    }

    /// Get content of the message
    #[inline]
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

    #[inline]
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
    #[inline]
    fn message_hash(&self) -> Result<Hash, MessageHashError> {
        let bytes = self.as_bytes()?;
        Ok(blake2b::digest_256(&bytes))
    }
}