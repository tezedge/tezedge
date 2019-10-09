// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use failure::_core::cmp::max;
use serde::{Deserialize, Serialize};

use storage::{IteratorMode, persistent::DatabaseWithSchema, StorageError};
use storage::persistent::{Codec, Schema, SchemaError};

// --- Storing result in Rocks DB --- //
pub type EventStorageDatabase = dyn DatabaseWithSchema<EventStorage> + Sync + Send;
pub type EventPayloadStorageDatabase = dyn DatabaseWithSchema<EventPayloadStorage> + Sync + Send;

#[derive(Clone)]
pub struct EventStorage {
    db: Arc<EventStorageDatabase>,
}

impl EventStorage {
    pub fn new(db: Arc<EventStorageDatabase>) -> Self {
        Self { db }
    }

    #[inline]
    pub fn put_event(&mut self, ts: u64, record_meta: &Event) -> Result<(), StorageError> {
        self.db.put(&RocksStamp(ts), record_meta)
            .map_err(StorageError::from)
    }

    pub fn count_events(&self) -> Result<usize, StorageError> {
        let iter = self.db.iterator(IteratorMode::End)?;
        let mut ret = 0;
        for (key, _) in iter {
            if let Ok(stamp) = key {
                ret = max(stamp.0, ret);
            }
        }
        Ok(ret as usize)
    }
}

impl Schema for EventStorage {
    const COLUMN_FAMILY_NAME: &'static str = "event_storage";
    type Key = RocksStamp;
    type Value = Event;
}

#[derive(Clone)]
pub struct EventPayloadStorage {
    db: Arc<EventPayloadStorageDatabase>,
}

impl EventPayloadStorage {
    pub fn new(db: Arc<EventPayloadStorageDatabase>) -> Self {
        Self { db }
    }

    pub fn put_record(&mut self, ts: u64, record: &Vec<u8>) -> Result<(), StorageError> {
        self.db.put(&RocksStamp(ts), record)
            .map_err(StorageError::from)
    }
}

impl Schema for EventPayloadStorage {
    const COLUMN_FAMILY_NAME: &'static str = "event_payload_storage";
    type Key = RocksStamp;
    type Value = Vec<u8>;
}

// --- Record implementation --- //

/// Simple counting identification stamp, denoted by order of messages
/// implementing the codec trait for simple database serialization.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq)]
pub struct RocksStamp(u64);

impl From<u64> for RocksStamp {
    fn from(val: u64) -> Self {
        Self(val)
    }
}

impl Codec for RocksStamp {
    #[inline]
    fn decode(data: &[u8]) -> Result<Self, SchemaError> {
        if data.len() < 8 {
            Err(SchemaError::DecodeError)
        } else {
            let mut buffer: [u8; 8] = Default::default();
            buffer.copy_from_slice(&data[0..8]);
            Ok(Self(u64::from_le_bytes(buffer)))
        }
    }

    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        Ok(self.0.to_le_bytes().to_vec())
    }
}

/// Part of the Record Meta, denoting content type in the Record storage
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum EventType {
    /// Connection to new peer was accepted. No record in Record Storage should correspond to
    /// this meta message
    PeerCreated,
    /// Full connection to peer was established. Record should contains a string with public
    /// key of connected peer.
    PeerBootstrapped,
    /// Node received a message from a peer. Record should contain raw message, which
    /// should be deserialized manually.
    PeerReceivedMessage,
}

impl EventType {
    fn from_u8(val: u8) -> Option<Self> {
        use EventType::*;

        if val == PeerCreated as u8 {
            Some(PeerCreated)
        } else if val == PeerBootstrapped as u8 {
            Some(PeerBootstrapped)
        } else if val == PeerReceivedMessage as u8 {
            Some(PeerReceivedMessage)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
/// Record metadata, describing incoming message.
pub struct Event {
    /// Description of type of incoming message, and stored format.
    pub record_type: EventType,
    /// Relative time in milliseconds, denoting when message came.
    pub timestamp: u64,
    /// Representation of an peer actor, to whom belongs the message
    pub peer_id: String,
}

impl Codec for Event {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() < 1 {
            return Err(SchemaError::DecodeError);
        }
        let record_type = bytes.first().ok_or(SchemaError::DecodeError)?;
        let record_type = EventType::from_u8(record_type.clone()).ok_or(SchemaError::DecodeError)?;
        let mut ts_buffer: [u8; 8] = Default::default();
        ts_buffer.copy_from_slice(&bytes[1..9]);
        let timestamp = u64::from_le_bytes(ts_buffer);
        let peer_id = String::from_utf8(bytes[9..].to_vec()).map_err(|_| SchemaError::DecodeError)?;
        Ok(Self { record_type, timestamp, peer_id })
    }

    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        // 1 byte event_type, 8 bytes (little endian) timestamp, rest is peer name (UTF-8)
        let mut ret = Vec::with_capacity(1 + 8 + self.peer_id.as_bytes().len());
        ret.push(self.record_type as u8);
        ret.extend_from_slice(&self.timestamp.to_le_bytes());
        ret.extend_from_slice(self.peer_id.as_bytes());
        Ok(ret)
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use super::*;

    #[test]
    fn rockstamp_encoded_equals_decoded() -> Result<(), Error> {
        let expected = RocksStamp(42);
        let encoded_bytes = expected.encode()?;
        assert_eq!(expected, RocksStamp::decode(&encoded_bytes)?);
        Ok(())
    }

    #[test]
    fn event_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Event {
            record_type: EventType::PeerCreated,
            timestamp: 0,
            peer_id: "peer-testing".to_string(),
        };
        let encoded_bytes = expected.encode()?;
        assert_eq!(expected, Event::decode(&encoded_bytes)?);
        Ok(())
    }
}