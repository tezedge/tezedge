// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_context::channel::ContextAction;
use tezos_encoding::hash::{BlockHash, OperationHash, HashType};

use crate::persistent::{Codec, DatabaseWithSchema, Schema, SchemaError};
use crate::StorageError;
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};

pub type ContextKeyHash = Vec<u8>;
pub type ContextStorageDatabase = dyn DatabaseWithSchema<ContextStorage> + Sync + Send;

pub struct ContextStorage {
    db: Arc<ContextStorageDatabase>
}

impl ContextStorage {

    pub fn new(db: Arc<ContextStorageDatabase>) -> Self {
        ContextStorage { db }
    }

    #[inline]
    pub fn put(&mut self, key: &ContextRecordKey, value: &ContextRecordValue) -> Result<(), StorageError> {
        self.db.put(key, value)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get(&self, key: &ContextRecordKey) -> Result<Option<ContextRecordValue>, StorageError> {
        self.db.get(key)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get_by_block_hash(&self, block_hash: &BlockHash) -> Result<Vec<ContextRecordValue>, StorageError> {
        let key = ContextRecordKey {
            block_hash: block_hash.clone(),
            key_hash: BLANK_KEY_HASH.to_vec(),
            operation_hash: None,
            ordinal_id: 0,
        };

        let mut values = vec![];
        for (_key, value) in self.db.prefix_iterator(&key)? {
            values.push(value?);
        }

        Ok(values)
    }
}

/// Key for a specific action stored in a database.
#[derive(PartialEq, Debug)]
pub struct ContextRecordKey {
    block_hash: BlockHash,
    key_hash: ContextKeyHash,
    operation_hash: Option<OperationHash>,
    /// This ID is used to order actions in a operation.
    /// It's uniqueness is not guaranteed outside of the context bound.
    ordinal_id: u32
}

impl ContextRecordKey {
    pub fn new(block_hash: &BlockHash, operation_hash: &Option<OperationHash>, key: &[String], ordinal_id: u32) -> Self {
        ContextRecordKey {
            block_hash: block_hash.clone(),
            operation_hash: operation_hash.clone(),
            key_hash: crypto::blake2b::digest_256(key.join(".").as_bytes()),
            ordinal_id
        }
    }
}

const LEN_KEY_HASH: usize = 32;
const LEN_BLOCK_HASH: usize = HashType::BlockHash.size();
const LEN_OPERATION_HASH: usize = HashType::OperationHash.size();
const LEN_ORDINAL_ID: usize = mem::size_of::<u32>();

const IDX_BLOCK_HASH: usize = 0;
const IDX_ORDINAL_ID: usize = IDX_BLOCK_HASH + LEN_BLOCK_HASH;
const IDX_KEY_HASH: usize = IDX_ORDINAL_ID + LEN_ORDINAL_ID;
const IDX_OPERATION_HASH: usize = IDX_KEY_HASH + LEN_KEY_HASH;

const LEN_RECORD_KEY: usize = LEN_BLOCK_HASH + LEN_KEY_HASH + LEN_OPERATION_HASH + LEN_ORDINAL_ID;
const BLANK_OPERATION_HASH: [u8; LEN_OPERATION_HASH] = [0; LEN_OPERATION_HASH];
const BLANK_KEY_HASH: [u8; LEN_KEY_HASH] = [0; LEN_KEY_HASH];

/// Codec for `RecordKey`
///
/// * bytes layout `[block_hash(32)][ordinal_id(4)][key_hash(32)][operation_hash(32)]`
impl Codec for ContextRecordKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if LEN_RECORD_KEY == bytes.len() {
            // block header hash
            let block_hash = bytes[IDX_BLOCK_HASH..IDX_BLOCK_HASH + LEN_BLOCK_HASH].to_vec();
            // ordinal_id
            let mut ordinal_id_bytes: [u8; 4] = Default::default();
            ordinal_id_bytes.copy_from_slice(&bytes[IDX_ORDINAL_ID..IDX_ORDINAL_ID + LEN_ORDINAL_ID]);
            let ordinal_id = u32::from_le_bytes(ordinal_id_bytes);
            // key hash
            let key_hash = bytes[IDX_KEY_HASH..IDX_KEY_HASH + LEN_KEY_HASH].to_vec();
            // operation hash
            let operation_hash = bytes[IDX_OPERATION_HASH..IDX_OPERATION_HASH + LEN_OPERATION_HASH].to_vec();
            let operation_hash = if operation_hash == BLANK_OPERATION_HASH {
                None
            } else {
                Some(operation_hash)
            };

            Ok(ContextRecordKey { block_hash, operation_hash, key_hash, ordinal_id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(LEN_RECORD_KEY);
        // block header hash
        result.extend(&self.block_hash);
        // ordinal
        result.extend(&self.ordinal_id.to_le_bytes());
        // key hash
        result.extend(&self.key_hash);
        // operation hash
        match &self.operation_hash {
            Some(operation_hash) => result.extend(operation_hash),
            None => result.extend(&BLANK_OPERATION_HASH),
        }
        assert_eq!(result.len(), LEN_RECORD_KEY, "Result length mismatch");
        Ok(result)
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

/// Codec for `RecordValue`
impl crate::persistent::BincodeEncoded for ContextRecordValue { }

impl Schema for ContextStorage {
    const COLUMN_FAMILY_NAME: &'static str = "context_storage";
    type Key = ContextRecordKey;
    type Value = ContextRecordValue;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(LEN_BLOCK_HASH));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use tezos_encoding::hash::{HashType, HashEncoding};

    use super::*;

    #[test]
    fn context_record_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextRecordKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            key_hash: vec![60; LEN_KEY_HASH],
            operation_hash: Some(vec![27; LEN_OPERATION_HASH]),
            ordinal_id: 6548654
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextRecordKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_key_blank_operation_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextRecordKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            key_hash: vec![60; LEN_KEY_HASH],
            operation_hash: None,
            ordinal_id: 176105218
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextRecordKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_get_values_by_block_hash() -> Result<(), Error> {
        use rocksdb::{Options, DB};

        let path = "__ctx_storage_get_by_block_hash";
        if std::path::Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![ContextStorage::cf_descriptor()]).unwrap();

            let block_hash_1 = HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
            let block_hash_2 = HashEncoding::new(HashType::BlockHash).string_to_bytes("BLaf78njreWdt2WigJjM9e3ecEdVKm5ehahUfYBKvcWvZ8vfTcJ")?;
            let key_1_0 = ContextRecordKey { block_hash: block_hash_1.clone(), ordinal_id: 0, operation_hash: Some(BLANK_OPERATION_HASH.to_vec()), key_hash: BLANK_KEY_HASH.to_vec() };
            let value_1_0 = ContextRecordValue { action: ContextAction::Set { key: vec!("hello".to_string(), "this".to_string(), "is".to_string(), "dog".to_string()), value: vec![10, 200], operation_hash: None, block_hash: Some(block_hash_1.clone()), context_hash: None } };
            let key_1_1 = ContextRecordKey { block_hash: block_hash_1.clone(), ordinal_id: 1, operation_hash: Some(BLANK_OPERATION_HASH.to_vec()), key_hash: BLANK_KEY_HASH.to_vec() };
            let value_1_1 = ContextRecordValue { action: ContextAction::Set { key: vec!("hello".to_string(), "world".to_string()), value: vec![11, 200], operation_hash: None, block_hash: Some(block_hash_1.clone()), context_hash: None } };
            let key_2_0 = ContextRecordKey { block_hash: block_hash_2.clone(), ordinal_id: 0, operation_hash: Some(BLANK_OPERATION_HASH.to_vec()), key_hash: BLANK_KEY_HASH.to_vec() };
            let value_2_0 = ContextRecordValue { action: ContextAction::Set { key: vec!("nice".to_string(), "to meet you".to_string()), value: vec![20, 200], operation_hash: None, block_hash: Some(block_hash_2.clone()), context_hash: None } };

            let mut storage = ContextStorage::new(Arc::new(db));
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

        assert!(DB::destroy(&opts, path).is_ok());
        Ok(())
    }
}