// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_context::{ContextKey, ContextValue};
use tezos_encoding::hash::{ChainId, ContextHash};

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
    pub fn put(&mut self, key: &RecordKey, value: &RecordValue) -> Result<(), StorageError> {
        self.db.put(key, value)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get(&self, key: &RecordKey) -> Result<Option<RecordValue>, StorageError> {
        self.db.get(key)
            .map_err(StorageError::from)
    }
}





#[derive(PartialEq, Debug)]
pub struct RecordKey {
    chain_id: ChainId,
    level: i32,
    context_hash: ContextHash,
    key_hash: ContextKeyHash,
}

impl RecordKey {
    pub fn new(chain_id: &ChainId, level: i32, context_hash: &ContextHash, key: &[String]) -> Self {
        RecordKey {
            chain_id: chain_id.clone(),
            level,
            context_hash: context_hash.clone(),
            key_hash: crypto::blake2b::digest_256(key.join(".").as_bytes())
        }
    }
}

const LEN_CHAIN_ID: usize = 4;
const LEN_LEVEL: usize = std::mem::size_of::<i32>();
const LEN_CONTEXT_HASH: usize = 32;
const LEN_KEY_HASH: usize = 32;

const LEN_RECORD_KEY: usize = LEN_CHAIN_ID + LEN_LEVEL + LEN_CONTEXT_HASH + LEN_KEY_HASH;

const IDX_CHAIN_ID: usize = 0;
const IDX_LEVEL: usize = IDX_CHAIN_ID + LEN_CHAIN_ID;
const IDX_CONTEXT_HASH: usize = IDX_LEVEL + LEN_LEVEL;
const IDX_KEY_HASH: usize = IDX_CONTEXT_HASH + LEN_CONTEXT_HASH;

/// Codec for `RecordKey`
///
/// * bytes layout `[chain_id(4)][level(4)][context_hash(32)][key(32)]`
impl Codec for RecordKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if LEN_RECORD_KEY == bytes.len() {
            let chain_id = bytes[IDX_CHAIN_ID..IDX_CHAIN_ID + LEN_CHAIN_ID].to_vec();
            let context_hash = bytes[IDX_CONTEXT_HASH..IDX_CONTEXT_HASH + LEN_CONTEXT_HASH].to_vec();
            let key_hash = bytes[IDX_KEY_HASH..IDX_KEY_HASH + LEN_KEY_HASH].to_vec();

            let mut level_bytes: [u8; LEN_LEVEL] = Default::default();
            level_bytes.copy_from_slice(&bytes[IDX_LEVEL..IDX_LEVEL + LEN_LEVEL]);
            let level = i32::from_le_bytes(level_bytes);

            Ok(RecordKey { chain_id, level, context_hash, key_hash })
        } else {
            Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(LEN_RECORD_KEY);
        result.extend(&self.chain_id);
        result.extend(&self.level.to_le_bytes());
        result.extend(&self.context_hash);
        result.extend(&self.key_hash);
        assert_eq!(result.len(), LEN_RECORD_KEY, "Result length mismatch");
        Ok(result)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct RecordValue {
    context_hash: Option<ContextHash>,
    key: ContextKey,
    value: ContextValue
}

impl RecordValue {
    pub fn new(context_hash: Option<ContextHash>, key: ContextKey, value: ContextValue) -> Self {
        RecordValue {
            context_hash,
            key, value
        }
    }
}

/// Codec for `RecordValue`
impl crate::persistent::BincodeEncoded for RecordValue { }

impl Schema for ContextStorage {
    const COLUMN_FAMILY_NAME: &'static str = "context_storage";
    type Key = RecordKey;
    type Value = RecordValue;
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use super::*;

    #[test]
    fn context_record_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = RecordKey {
            chain_id: vec![91; LEN_CHAIN_ID],
            level: 93_422,
            context_hash: vec![43; LEN_CONTEXT_HASH],
            key_hash: vec![60; LEN_KEY_HASH],
        };
        let encoded_bytes = expected.encode()?;
        let decoded = RecordKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_value_encoded_equals_decoded() -> Result<(), Error> {
        let expected = RecordValue {
            context_hash: Some(vec![43; LEN_CONTEXT_HASH]),
            key: vec!["this_is_key".to_string(); 7],
            value: vec![45; 1024],
        };
        let encoded_bytes = expected.encode()?;
        let decoded = RecordValue::decode(&encoded_bytes)?;
        assert_eq!(expected, decoded);


        let expected = RecordValue {
            context_hash: None,
            key: vec!["this_is_key".to_string(); 7],
            value: vec![45; 0],
        };
        let encoded_bytes = expected.encode()?;
        let decoded = RecordValue::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }
}