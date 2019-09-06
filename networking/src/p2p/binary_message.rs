use serde::Serialize;
use serde::de::DeserializeOwned;
use failure::Fail;

use crypto::blake2b;
use tezos_encoding::binary_reader::BinaryReader;
use tezos_encoding::binary_writer::BinaryWriter;
use tezos_encoding::de;
use tezos_encoding::de::from_value as deserialize_from_value;
use tezos_encoding::json_writer::JsonWriter;
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding::ser;
use tezos_encoding::hash::Hash;
use crate::p2p::binary_message::MessageHashError::SerializationError;

pub const MESSAGE_LENGTH_FIELD_SIZE: usize = 2;

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
///
/// Example:
/// ```rust
/// use crate::networking::p2p::binary_message::RawBinaryMessage;
///
/// let raw: RawBinaryMessage = hex::decode("000600108eceda2f").unwrap().into();
///
/// let all_received_bytes = hex::decode("000600108eceda2f").unwrap();
/// assert_eq!(raw.get_raw(), &all_received_bytes);
///
/// let actual_message_bytes = hex::decode("00108eceda2f").unwrap();
/// assert_eq!(&raw.get_contents().to_vec(), &actual_message_bytes);
/// ```
pub struct RawBinaryMessage(Vec<u8>);

impl RawBinaryMessage {

    pub fn get_raw(&self) -> &Vec<u8> {
        &self.0
    }

    pub fn get_contents(&self) -> &[u8] {
        &self.0[MESSAGE_LENGTH_FIELD_SIZE..]
    }
}

impl From<Vec<u8>> for RawBinaryMessage {
    fn from(data: Vec<u8>) -> Self {
        RawBinaryMessage(data)
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