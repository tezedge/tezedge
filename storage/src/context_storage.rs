// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_context::channel::ContextAction;
use tezos_encoding::hash::{BlockHash, OperationHash};

use crate::persistent::{Codec, DatabaseWithSchema, Schema, SchemaError};
use crate::StorageError;

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
const LEN_BLOCK_HASH: usize = 32;
const LEN_OPERATION_HASH: usize = 32;
const LEN_ORDINAL_ID: usize = mem::size_of::<u32>();

const IDX_KEY_HASH: usize = 0;
const IDX_BLOCK_HASH: usize = IDX_KEY_HASH + LEN_KEY_HASH;
const IDX_OPERATION_HASH: usize = IDX_BLOCK_HASH + LEN_BLOCK_HASH;
const IDX_ORDINAL_ID: usize = IDX_OPERATION_HASH + LEN_OPERATION_HASH;

const LEN_RECORD_KEY: usize = LEN_BLOCK_HASH + LEN_KEY_HASH + LEN_OPERATION_HASH + LEN_ORDINAL_ID;
const BLANK_OPERATION_HASH: [u8; LEN_OPERATION_HASH] = [0; LEN_OPERATION_HASH];

/// Codec for `RecordKey`
///
/// * bytes layout `[key(32)][block_hash(32)][ordinal_id(4)]`
impl Codec for ContextRecordKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if LEN_RECORD_KEY == bytes.len() {
            let key_hash = bytes[IDX_KEY_HASH..IDX_KEY_HASH + LEN_KEY_HASH].to_vec();
            let block_hash = bytes[IDX_BLOCK_HASH..IDX_BLOCK_HASH + LEN_BLOCK_HASH].to_vec();
            let operation_hash = bytes[IDX_OPERATION_HASH..IDX_OPERATION_HASH + LEN_OPERATION_HASH].to_vec();
            let operation_hash = if operation_hash == BLANK_OPERATION_HASH {
                None
            } else {
                Some(operation_hash)
            };
            // ordinal_id
            let mut ordinal_id_bytes: [u8; 4] = Default::default();
            ordinal_id_bytes.copy_from_slice(&bytes[IDX_ORDINAL_ID..IDX_ORDINAL_ID + LEN_ORDINAL_ID]);
            let ordinal_id = u32::from_le_bytes(ordinal_id_bytes);
            
            Ok(ContextRecordKey { block_hash, operation_hash, key_hash, ordinal_id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(LEN_RECORD_KEY);
        result.extend(&self.key_hash);
        result.extend(&self.block_hash);
        match &self.operation_hash {
            Some(operation_hash) => result.extend(operation_hash),
            None => result.extend(&BLANK_OPERATION_HASH),
        }
        result.extend(&self.ordinal_id.to_le_bytes());
        assert_eq!(result.len(), LEN_RECORD_KEY, "Result length mismatch");
        Ok(result)
    }
}

#[derive(Serialize, Deserialize)]
pub struct ContextRecordValue {
    action: ContextAction,
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
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use tezos_encoding::hash::HashType;

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
}