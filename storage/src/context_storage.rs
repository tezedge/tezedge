// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};
use serde::{Deserialize, Serialize};

use tezos_context::channel::ContextAction;
use tezos_encoding::hash::{BlockHash, HashType, OperationHash};

use crate::persistent::{CommitLogs, CommitLogSchema, CommitLogWithSchema, DatabaseWithSchema, Decoder, Encoder, KeyValueSchema, Location, SchemaError};
use crate::persistent::commit_log::fold_consecutive_locations;
use crate::StorageError;

pub type ContextStorageCommitLog = dyn CommitLogWithSchema<ContextStorage> + Sync + Send;

/// Holds all actions received from a tezos context.
/// Action is created every time a context is modified.
pub struct ContextStorage {
    context_primary_index: ContextPrimaryIndex,
    clog: Arc<ContextStorageCommitLog>
}

impl ContextStorage {

    pub fn new(db: Arc<rocksdb::DB>, clog: Arc<CommitLogs>) -> Self {
        Self {
            context_primary_index: ContextPrimaryIndex::new(db),
            clog
        }
    }

    #[inline]
    pub fn put(&mut self, key: &ContextPrimaryIndexKey, value: &ContextRecordValue) -> Result<(), StorageError> {
        self.clog.append(value)
            .map_err(StorageError::from)
            .and_then(|location| self.context_primary_index.put(key, &location))
    }

    #[inline]
    pub fn get(&self, key: &ContextPrimaryIndexKey) -> Result<Option<ContextRecordValue>, StorageError> {
        self.context_primary_index.get(key)?
            .map(|location| self.get_record_by_location(&location))
            .transpose()
    }

    #[inline]
    pub fn get_by_block_hash(&self, block_hash: &BlockHash) -> Result<Vec<ContextRecordValue>, StorageError> {
        self.context_primary_index.get_by_block_hash(block_hash)
            .and_then(|locations| self.get_records_by_locations(&locations))
    }

    /// Retrieve record value from commit log or return error if value is not present.
    #[inline]
    fn get_record_by_location(&self, location: &Location) -> Result<ContextRecordValue, StorageError> {
        self.clog.get(location).map_err(StorageError::from).or(Err(StorageError::MissingKey))
    }

    /// Retrieve record values in batch
    fn get_records_by_locations(&self, locations: &[Location]) -> Result<Vec<ContextRecordValue>, StorageError> {
        match locations.len() {
            0 => Ok(Vec::with_capacity(0)),
            1 => Ok(vec![self.get_record_by_location(&locations[0])?]),
            _ => {
                let records = fold_consecutive_locations(locations)
                    .iter()
                    .map(|range| self.clog.get_range(range).map_err(StorageError::from))
                    .collect::<Result<Vec<Vec<_>>, _>>()?;
                Ok(records.into_iter().flatten().collect())
            }
        }
    }
}

impl CommitLogSchema for ContextStorage {
    type Value = ContextRecordValue;

    #[inline]
    fn name() -> &'static str {
        "context_storage"
    }
}

#[derive(Serialize, Deserialize)]
pub struct ContextRecordValue {
    pub action: ContextAction,
}

impl ContextRecordValue {
    pub fn new(action: ContextAction) -> Self {
        Self { action }
    }
}

/// Codec for `ContextRecordValue`
impl crate::persistent::BincodeEncoded for ContextRecordValue { }



/// Index block data as `context_primary_key -> location`.
pub struct ContextPrimaryIndex {
    db: Arc<ContextPrimaryIndexDatabase>,
}

pub type ContextKeyHash = Vec<u8>;
pub type ContextPrimaryIndexDatabase = dyn DatabaseWithSchema<ContextPrimaryIndex> + Sync + Send;

impl ContextPrimaryIndex {

    fn new(db: Arc<ContextPrimaryIndexDatabase>) -> Self {
        Self { db }
    }

    #[inline]
    fn put(&mut self, key: &ContextPrimaryIndexKey, value: &Location) -> Result<(), StorageError> {
        self.db.put(key, value)
            .map_err(StorageError::from)
    }

    #[inline]
    fn get(&self, key: &ContextPrimaryIndexKey) -> Result<Option<Location>, StorageError> {
        self.db.get(key)
            .map_err(StorageError::from)
    }

    #[inline]
    fn get_by_block_hash(&self, block_hash: &BlockHash) -> Result<Vec<Location>, StorageError> {
        let key = ContextPrimaryIndexKey::from_block_hash(block_hash);

        self.db.prefix_iterator(&key)?
            .map(|(_, value)| value.map_err(StorageError::from))
            .collect()
    }
}

impl KeyValueSchema for ContextPrimaryIndex {
    type Key = ContextPrimaryIndexKey;
    type Value = Location;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(ContextPrimaryIndexKey::LEN_BLOCK_HASH));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "context_storage"
    }
}

/// Key for a specific action stored in a database.
#[derive(PartialEq, Debug)]
pub struct ContextPrimaryIndexKey {
    block_hash: BlockHash,
    key_hash: ContextKeyHash,
    operation_hash: Option<OperationHash>,
    /// This ID is used to order actions in a operation.
    /// It's uniqueness is not guaranteed outside of the context bound.
    ordinal_id: u32
}

impl ContextPrimaryIndexKey {

    const LEN_KEY_HASH: usize = 32;
    const LEN_BLOCK_HASH: usize = HashType::BlockHash.size();
    const LEN_OPERATION_HASH: usize = HashType::OperationHash.size();
    const LEN_ORDINAL_ID: usize = mem::size_of::<u32>();

    const IDX_BLOCK_HASH: usize = 0;
    const IDX_ORDINAL_ID: usize = Self::IDX_BLOCK_HASH + Self::LEN_BLOCK_HASH;
    const IDX_KEY_HASH: usize = Self::IDX_ORDINAL_ID + Self::LEN_ORDINAL_ID;
    const IDX_OPERATION_HASH: usize = Self::IDX_KEY_HASH + Self::LEN_KEY_HASH;

    const LEN_RECORD_KEY: usize = Self::LEN_BLOCK_HASH + Self::LEN_KEY_HASH + Self::LEN_OPERATION_HASH + Self::LEN_ORDINAL_ID;
    const BLANK_OPERATION_HASH: [u8; Self::LEN_OPERATION_HASH] = [0; Self::LEN_OPERATION_HASH];
    const BLANK_KEY_HASH: [u8; Self::LEN_KEY_HASH] = [0; Self::LEN_KEY_HASH];

    pub fn new(block_hash: &BlockHash, operation_hash: &Option<OperationHash>, key: &[String], ordinal_id: u32) -> Self {
        Self {
            block_hash: block_hash.clone(),
            operation_hash: operation_hash.clone(),
            key_hash: crypto::blake2b::digest_256(key.join(".").as_bytes()),
            ordinal_id,
        }
    }

    /// This is useful only when using prefix iterator to retrieve
    /// actions belonging to the same block.
    fn from_block_hash(block_hash: &BlockHash) -> Self {
        Self {
            block_hash: block_hash.clone(),
            key_hash: ContextPrimaryIndexKey::BLANK_KEY_HASH.to_vec(),
            operation_hash: None,
            ordinal_id: 0,
        }
    }
}

/// Decoder for `ContextPrimaryIndexKey`
///
/// * bytes layout `[block_hash(32)][ordinal_id(4)][key_hash(32)][operation_hash(32)]`
impl Decoder for ContextPrimaryIndexKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if Self::LEN_RECORD_KEY == bytes.len() {
            // block header hash
            let block_hash = bytes[Self::IDX_BLOCK_HASH..Self::IDX_BLOCK_HASH + Self::LEN_BLOCK_HASH].to_vec();
            // ordinal_id
            let mut ordinal_id_bytes: [u8; 4] = Default::default();
            ordinal_id_bytes.copy_from_slice(&bytes[Self::IDX_ORDINAL_ID..Self::IDX_ORDINAL_ID + Self::LEN_ORDINAL_ID]);
            let ordinal_id = u32::from_be_bytes(ordinal_id_bytes);
            // key hash
            let key_hash = bytes[Self::IDX_KEY_HASH..Self::IDX_KEY_HASH + Self::LEN_KEY_HASH].to_vec();
            // operation hash
            let operation_hash = bytes[Self::IDX_OPERATION_HASH..Self::IDX_OPERATION_HASH + Self::LEN_OPERATION_HASH].to_vec();
            let operation_hash = if operation_hash == Self::BLANK_OPERATION_HASH {
                None
            } else {
                Some(operation_hash)
            };

            Ok(ContextPrimaryIndexKey { block_hash, operation_hash, key_hash, ordinal_id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

/// Encoder for `ContextPrimaryIndexKey`
///
/// * bytes layout `[block_hash(32)][ordinal_id(4)][key_hash(32)][operation_hash(32)]`
impl Encoder for ContextPrimaryIndexKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(Self::LEN_RECORD_KEY);
        // block header hash
        result.extend(&self.block_hash);
        // ordinal
        result.extend(&self.ordinal_id.to_be_bytes());
        // key hash
        result.extend(&self.key_hash);
        // operation hash
        match &self.operation_hash {
            Some(operation_hash) => result.extend(operation_hash),
            None => result.extend(&Self::BLANK_OPERATION_HASH),
        }
        assert_eq!(result.len(), Self::LEN_RECORD_KEY, "Result length mismatch");
        Ok(result)
    }
}


#[cfg(test)]
mod tests {
    use failure::Error;

    use tezos_encoding::hash::{HashEncoding, HashType};

    use crate::persistent::{open_cl, open_db};

    use super::*;

    #[test]
    fn context_record_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            key_hash: vec![60; ContextPrimaryIndexKey::LEN_KEY_HASH],
            operation_hash: Some(vec![27; ContextPrimaryIndexKey::LEN_OPERATION_HASH]),
            ordinal_id: 6548654
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_key_blank_operation_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            key_hash: vec![60; ContextPrimaryIndexKey::LEN_KEY_HASH],
            operation_hash: None,
            ordinal_id: 176105218
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_get_values_by_block_hash() -> Result<(), Error> {
        use rocksdb::{Options, DB};

        let path = "__ctx_storage_get_by_block_hash";
        if std::path::Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        {
            let db = open_db(path, vec![ContextPrimaryIndex::descriptor()]).unwrap();
            let clog = open_cl(path, vec![ContextStorage::descriptor()])?;

            let block_hash_1 = HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
            let block_hash_2 = HashEncoding::new(HashType::BlockHash).string_to_bytes("BLaf78njreWdt2WigJjM9e3ecEdVKm5ehahUfYBKvcWvZ8vfTcJ")?;
            let key_1_0 = ContextPrimaryIndexKey { block_hash: block_hash_1.clone(), ordinal_id: 0, operation_hash: Some(ContextPrimaryIndexKey::BLANK_OPERATION_HASH.to_vec()), key_hash: ContextPrimaryIndexKey::BLANK_KEY_HASH.to_vec() };
            let value_1_0 = ContextRecordValue { action: ContextAction::Set { key: vec!("hello".to_string(), "this".to_string(), "is".to_string(), "dog".to_string()), value: vec![10, 200], operation_hash: None, block_hash: Some(block_hash_1.clone()), context_hash: None } };
            let key_1_1 = ContextPrimaryIndexKey { block_hash: block_hash_1.clone(), ordinal_id: 1, operation_hash: Some(ContextPrimaryIndexKey::BLANK_OPERATION_HASH.to_vec()), key_hash: ContextPrimaryIndexKey::BLANK_KEY_HASH.to_vec() };
            let value_1_1 = ContextRecordValue { action: ContextAction::Set { key: vec!("hello".to_string(), "world".to_string()), value: vec![11, 200], operation_hash: None, block_hash: Some(block_hash_1.clone()), context_hash: None } };
            let key_2_0 = ContextPrimaryIndexKey { block_hash: block_hash_2.clone(), ordinal_id: 0, operation_hash: Some(ContextPrimaryIndexKey::BLANK_OPERATION_HASH.to_vec()), key_hash: ContextPrimaryIndexKey::BLANK_KEY_HASH.to_vec() };
            let value_2_0 = ContextRecordValue { action: ContextAction::Set { key: vec!("nice".to_string(), "to meet you".to_string()), value: vec![20, 200], operation_hash: None, block_hash: Some(block_hash_2.clone()), context_hash: None } };

            let mut storage = ContextStorage::new(Arc::new(db), Arc::new(clog));
            storage.put(&key_1_0, &value_1_0)?;
            storage.put(&key_2_0, &value_2_0)?;
            storage.put(&key_1_1, &value_1_1)?;

            // block hash 1
            let values = storage.get_by_block_hash(&block_hash_1)?;
            assert_eq!(2, values.len(), "Was expecting vector of {} elements but instead found {}", 2, values.len());
            if let ContextAction::Set { value, .. } = &values[0].action {
                assert_eq!(&vec![10, 200], value);
            } else {
                panic!("Was expecting ContextAction::Set");
            }
            if let ContextAction::Set { value, .. } = &values[1].action {
                assert_eq!(&vec![11, 200], value);
            } else {
                panic!("Was expecting ContextAction::Set");
            }
            // block hash 2
            let values = storage.get_by_block_hash(&block_hash_2)?;
            assert_eq!(1, values.len(), "Was expecting vector of {} elements but instead found {}", 1, values.len());
            if let ContextAction::Set { value, .. } = &values[0].action {
                assert_eq!(&vec![20, 200], value);
            } else {
                panic!("Was expecting ContextAction::Set");
            }
        }

        assert!(DB::destroy(&Options::default(), path).is_ok());
        Ok(assert!(std::fs::remove_dir_all(path).is_ok()))
    }
}