use std::sync::Arc;
use serde::{Serialize, Deserialize};
use storage::{
    persistent::DatabaseWithSchema,
    StorageError,
};
use storage::persistent::{Schema, Codec, SchemaError};
use byteorder::{LittleEndian, WriteBytesExt, ReadBytesExt};

/// --- Storing result in Rocks DB --- ///
pub type RecordMetaStorageDatabase = dyn DatabaseWithSchema<RecordMetaStorage> + Sync + Send;
pub type RecordStorageDatabase = dyn DatabaseWithSchema<RecordStorage> + Sync + Send;

#[derive(Clone)]
pub struct RecordMetaStorage {
    db: Arc<RecordMetaStorageDatabase>,
}

impl RecordMetaStorage {
    pub fn new(db: Arc<RecordMetaStorageDatabase>) -> Self {
        Self { db }
    }

    #[inline]
    pub fn put_record_meta(&mut self, ts: f32, record_meta: &RecordMeta) -> Result<(), StorageError> {
        self.db.put(&RocksStamp(ts), record_meta)
            .map_err(StorageError::from)
    }
}

impl Schema for RecordMetaStorage {
    const COLUMN_FAMILY_NAME: &'static str = "record_meta_storage";
    type Key = RocksStamp;
    type Value = RecordMeta;
}

#[derive(Clone)]
pub struct RecordStorage {
    db: Arc<RecordStorageDatabase>,
}

impl RecordStorage {
    pub fn new(db: Arc<RecordStorageDatabase>) -> Self {
        Self { db }
    }

    pub fn put_record(&mut self, ts: f32, record: &Vec<u8>) -> Result<(), StorageError> {
        self.db.put(&RocksStamp(ts), record)
            .map_err(StorageError::from)
    }
}

impl Schema for RecordStorage {
    const COLUMN_FAMILY_NAME: &'static str = "record_storage";
    type Key = RocksStamp;
    type Value = Vec<u8>;
}

/// --- Record implementation --- ///
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RocksStamp(f32);

impl From<f32> for RocksStamp {
    fn from(val: f32) -> Self {
        Self(val)
    }
}

impl Codec for RocksStamp {
    #[inline]
    fn decode(data: &[u8]) -> Result<Self, SchemaError> {
        if data.len() < 4 {
            Err(SchemaError::DecodeError)
        } else {
            Ok(Self((&data[0..4]).read_f32::<LittleEndian>().map_err(|_| SchemaError::DecodeError)?))
        }
    }

    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut data = Vec::with_capacity(4);
        data.write_f32::<LittleEndian>(self.0).map_err(|_| SchemaError::EncodeError)?;
        Ok(data)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(u8)]
pub enum RecordType {
    PeerCreated,
    PeerBootstrapped,
    PeerReceivedMessage,
}

impl RecordType {
    fn from_u8(val: u8) -> Option<Self> {
        use RecordType::*;
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

#[derive(Debug, Clone)]
pub struct RecordMeta {
    pub record_type: RecordType,
    pub peer_id: String,
}

impl Codec for RecordMeta {
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if bytes.len() < 1 {
            return Err(SchemaError::DecodeError);
        }
        let record_type = bytes.first().ok_or(SchemaError::DecodeError)?;
        let record_type = RecordType::from_u8(record_type.clone()).ok_or(SchemaError::DecodeError)?;
        let peer_id = String::from_utf8(bytes[1..].to_vec()).map_err(|_| SchemaError::DecodeError)?;
        Ok(Self { record_type, peer_id })
    }

    #[inline]
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        // 1 byte record_type, 8 bytes (little endian) timestamp, rest is peer name (UTF-8)
        let mut ret = Vec::with_capacity(1 + 8 + self.peer_id.as_bytes().len());
        ret.push(self.record_type as u8);
        ret.extend_from_slice(self.peer_id.as_bytes());
        Ok(ret)
    }
}