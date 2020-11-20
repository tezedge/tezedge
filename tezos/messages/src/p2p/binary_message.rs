// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use bytes::{Buf, BufMut};
use failure::Fail;
use failure::_core::convert::TryFrom;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crypto::blake2b;
use crypto::hash::Hash;
use tezos_encoding::binary_reader::{BinaryReader, BinaryReaderError};
use tezos_encoding::binary_writer;
use tezos_encoding::de::from_value as deserialize_from_value;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::json_writer::JsonWriter;
use tezos_encoding::ser;

use crate::p2p::binary_message::MessageHashError::SerializationError;

/// Size in bytes of the content length field
pub const CONTENT_LENGTH_FIELD_BYTES: usize = 2;
/// Max allowed message length in bytes
pub const CONTENT_LENGTH_MAX: usize = u16::max_value() as usize;

/// This feature can provide cache mechanism for BinaryMessages.
/// Cache is used to reduce computation time of encoding/decoding process.
///
/// When we use cache (see macro [cached_data]):
/// - first time we read [from_bytes], original bytes are stored to cache and message/struct is constructed from bytes
/// - so next time we want to use/call [as_bytes], bytes are not calculated with encoding, but just returned from cache
///
/// e.g: this is used, when we receive data from p2p as bytes, and then store them also as bytes to storage and calculate count of bytes in monitoring
///
/// When we dont need cache (see macro [non_cached_data]):
///
/// e.g.: we we just want to read data from bytes and never convert back to bytes
///
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
        fn cache_reader(&self) -> Option<&dyn CacheReader>;
        fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter>;
    }

    #[derive(Clone, Default)]
    pub struct BinaryDataCache {
        data: Option<Vec<u8>>,
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
            D: Deserializer<'de>,
        {
            Ok(BinaryDataCache::default())
        }
    }

    impl fmt::Debug for BinaryDataCache {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self.get() {
                Some(data) => write!(
                    f,
                    "BinaryDataCache {{ has_value: true, len: {} }}",
                    data.len()
                ),
                None => write!(f, "BinaryDataCache {{ has_value: false }}"),
            }
        }
    }

    #[derive(Clone, PartialEq, Default)]
    pub struct NeverCache;

    impl<'de> Deserialize<'de> for NeverCache {
        fn deserialize<D>(_: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
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

    /// Adds implementation CachedData for given struct
    /// Struct should contains property [$property_cache_name] with cache struct, e.g. BinaryDataCache,
    /// usually this cache does not need to be serialized, so can be marked with [#[serde(skip_serializing)]]
    #[macro_export]
    macro_rules! cached_data {
        ($struct_name:ident, $property_cache_name:ident) => {
            impl $crate::p2p::binary_message::cache::CachedData for $struct_name {
                #[inline]
                fn cache_reader(
                    &self,
                ) -> Option<&dyn $crate::p2p::binary_message::cache::CacheReader> {
                    Some(&self.$property_cache_name)
                }

                #[inline]
                fn cache_writer(
                    &mut self,
                ) -> Option<&mut dyn $crate::p2p::binary_message::cache::CacheWriter> {
                    Some(&mut self.$property_cache_name)
                }
            }
        };
    }

    /// Adds empty non-caching implementation CachedData for given struct
    #[macro_export]
    macro_rules! non_cached_data {
        ($struct_name:ident) => {
            impl $crate::p2p::binary_message::cache::CachedData for $struct_name {
                #[inline]
                fn cache_reader(
                    &self,
                ) -> Option<&dyn $crate::p2p::binary_message::cache::CacheReader> {
                    None
                }

                #[inline]
                fn cache_writer(
                    &mut self,
                ) -> Option<&mut dyn $crate::p2p::binary_message::cache::CacheWriter> {
                    None
                }
            }
        };
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
    fn from_bytes<B: AsRef<[u8]>>(buf: B) -> Result<Self, BinaryReaderError>;
}

impl<T> BinaryMessage for T
where
    T: HasEncoding + cache::CachedData + DeserializeOwned + Serialize + Sized,
{
    #[inline]
    fn as_bytes(&self) -> Result<Vec<u8>, ser::Error> {
        // check cache at first
        if let Some(cache) = self.cache_reader() {
            if let Some(data) = cache.get() {
                return Ok(data);
            }
        }

        // if cache not configured or empty, resolve by encoding
        binary_writer::write(self, &Self::encoding())
    }

    #[inline]
    fn from_bytes<B: AsRef<[u8]>>(bytes: B) -> Result<Self, BinaryReaderError> {
        let bytes = bytes.as_ref();

        let value = BinaryReader::new().read(bytes, &Self::encoding())?;
        let mut myself: Self = deserialize_from_value(&value)?;
        if let Some(cache_writer) = myself.cache_writer() {
            cache_writer.put(bytes);
        }
        Ok(myself)
    }
}

/// Represents binary raw encoding received from peer node.
///
/// Difference from [`BinaryMessage`] is that it also contains [`CONTENT_LENGTH_FIELD_BYTES`] bytes
/// of information about how many bytes is the actual encoding.
pub struct BinaryChunk(Vec<u8>);

impl BinaryChunk {
    /// Create new `BinaryChunk` from input content.
    pub fn from_content(content: &[u8]) -> Result<BinaryChunk, BinaryChunkError> {
        if content.len() <= CONTENT_LENGTH_MAX {
            // add length
            let mut bytes = Vec::with_capacity(CONTENT_LENGTH_FIELD_BYTES + content.len());
            // adds MESSAGE_LENGTH_FIELD_SIZE -- 2 bytes with length of the content
            bytes.put_u16(content.len() as u16);
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
    #[fail(
        display = "Incorrect content size information. expected={}, actual={}",
        expected, actual
    )]
    IncorrectSizeInformation { expected: usize, actual: usize },
}

/// Convert `Vec<u8>` into `BinaryChunk`. It is required that input `Vec<u8>`
/// contains also information about the content length in its first 2 bytes.
impl TryFrom<Vec<u8>> for BinaryChunk {
    type Error = BinaryChunkError;

    fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
        if value.len() < CONTENT_LENGTH_FIELD_BYTES {
            Err(BinaryChunkError::MissingSizeInformation)
        } else if value.len() <= (CONTENT_LENGTH_MAX + CONTENT_LENGTH_FIELD_BYTES) {
            let expected_content_length =
                (&value[0..CONTENT_LENGTH_FIELD_BYTES]).get_u16() as usize;
            if (expected_content_length + CONTENT_LENGTH_FIELD_BYTES) == value.len() {
                Ok(BinaryChunk(value))
            } else {
                Err(BinaryChunkError::IncorrectSizeInformation {
                    expected: expected_content_length,
                    actual: value.len(),
                })
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
where
    T: HasEncoding + Serialize + Sized,
{
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
    SerializationError { error: ser::Error },
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

impl<T: BinaryMessage + cache::CachedData> MessageHash for T {
    #[inline]
    fn message_hash(&self) -> Result<Hash, MessageHashError> {
        let bytes = self.as_bytes()?;
        Ok(blake2b::digest_256(&bytes))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_binary_from_content() -> Result<(), failure::Error> {
        let chunk = BinaryChunk::from_content(&[])?.0;
        assert_eq!(CONTENT_LENGTH_FIELD_BYTES, chunk.len());
        assert_eq!(CONTENT_LENGTH_FIELD_BYTES, chunk.capacity());

        let chunk = BinaryChunk::from_content(&[1])?.0;
        assert_eq!(CONTENT_LENGTH_FIELD_BYTES + 1, chunk.len());
        assert_eq!(CONTENT_LENGTH_FIELD_BYTES + 1, chunk.capacity());

        let chunk =
            BinaryChunk::from_content(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15])?.0;
        assert_eq!(CONTENT_LENGTH_FIELD_BYTES + 15, chunk.len());
        assert_eq!(CONTENT_LENGTH_FIELD_BYTES + 15, chunk.capacity());

        let chunk = BinaryChunk::from_content(&[1; CONTENT_LENGTH_MAX])?.0;
        assert_eq!(CONTENT_LENGTH_FIELD_BYTES + CONTENT_LENGTH_MAX, chunk.len());
        assert_eq!(
            CONTENT_LENGTH_FIELD_BYTES + CONTENT_LENGTH_MAX,
            chunk.capacity()
        );
        Ok(())
    }
}
