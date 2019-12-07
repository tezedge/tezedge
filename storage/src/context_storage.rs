// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType};
use tezos_context::channel::ContextAction;

use crate::persistent::{CommitLogSchema, CommitLogWithSchema, Decoder, Encoder, KeyValueSchema, KeyValueStoreWithSchema, Location, PersistentStorage, SchemaError};
use crate::persistent::commit_log::fold_consecutive_locations;
use crate::persistent::sequence::{SequenceGenerator, SequenceNumber};
use crate::StorageError;

pub type ContextStorageCommitLog = dyn CommitLogWithSchema<ContextStorage> + Sync + Send;

/// Holds all actions received from a tezos context.
/// Action is created every time a context is modified.
pub struct ContextStorage {
    context_primary_index: ContextPrimaryIndex,
    clog: Arc<ContextStorageCommitLog>,
    generator: Arc<SequenceGenerator>,
}

impl ContextStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            context_primary_index: ContextPrimaryIndex::new(persistent_storage.kv()),
            clog: persistent_storage.clog(),
            generator: persistent_storage.seq().generator(Self::name()),
        }
    }

    #[inline]
    pub fn put(&mut self, block_hash: &BlockHash, value: &ContextRecordValue) -> Result<(), StorageError> {
        self.clog.append(value)
            .map_err(StorageError::from)
            .and_then(|location| self.generator.next().map(|id| (location, id)).map_err(StorageError::from))
            .and_then(|(location, id)| self.context_primary_index.put(&ContextPrimaryIndexKey::new(block_hash, id), &location))
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
    kv: Arc<ContextPrimaryIndexKV>,
}

pub type ContextKeyHash = Vec<u8>;
pub type ContextPrimaryIndexKV = dyn KeyValueStoreWithSchema<ContextPrimaryIndex> + Sync + Send;

impl ContextPrimaryIndex {
    fn new(kv: Arc<ContextPrimaryIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(&mut self, key: &ContextPrimaryIndexKey, value: &Location) -> Result<(), StorageError> {
        self.kv.put(key, value)
            .map_err(StorageError::from)
    }

    #[inline]
    fn get_by_block_hash(&self, block_hash: &BlockHash) -> Result<Vec<Location>, StorageError> {
        let key = ContextPrimaryIndexKey::from_block_hash(block_hash);

        self.kv.prefix_iterator(&key)?
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
    id: SequenceNumber,
}

impl ContextPrimaryIndexKey {
    const LEN_BLOCK_HASH: usize = HashType::BlockHash.size();
    const LEN_ID: usize = mem::size_of::<u64>();

    const IDX_BLOCK_HASH: usize = 0;
    const IDX_ID: usize = Self::IDX_BLOCK_HASH + Self::LEN_BLOCK_HASH;

    const LEN_RECORD_KEY: usize = Self::LEN_BLOCK_HASH + Self::LEN_ID;

    pub fn new(block_hash: &BlockHash, id: SequenceNumber) -> Self {
        Self {
            block_hash: block_hash.clone(),
            id,
        }
    }

    /// This is useful only when using prefix iterator to retrieve
    /// actions belonging to the same block.
    fn from_block_hash(block_hash: &BlockHash) -> Self {
        Self {
            block_hash: block_hash.clone(),
            id: 0,
        }
    }
}

/// Decoder for `ContextPrimaryIndexKey`
///
/// * bytes layout `[block_hash(32)][id(8)]`
impl Decoder for ContextPrimaryIndexKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if Self::LEN_RECORD_KEY == bytes.len() {
            // block header hash
            let block_hash = bytes[Self::IDX_BLOCK_HASH..Self::IDX_BLOCK_HASH + Self::LEN_BLOCK_HASH].to_vec();
            // id
            let mut id_bytes: [u8; Self::LEN_ID] = Default::default();
            id_bytes.copy_from_slice(&bytes[Self::IDX_ID..Self::IDX_ID + Self::LEN_ID]);
            let id = u64::from_be_bytes(id_bytes);
            Ok(ContextPrimaryIndexKey { block_hash, id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

/// Encoder for `ContextPrimaryIndexKey`
///
/// * bytes layout `[block_hash(32)][id(8)]`
impl Encoder for ContextPrimaryIndexKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(Self::LEN_RECORD_KEY);
        // block header hash
        result.extend(&self.block_hash);
        // ordinal
        result.extend(&self.id.to_be_bytes());
        assert_eq!(result.len(), Self::LEN_RECORD_KEY, "Result length mismatch");
        Ok(result)
    }
}


#[cfg(test)]
mod tests {
    use failure::Error;

    use crypto::hash::HashType;

    use super::*;

    #[test]
    fn context_record_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 6548654
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_key_blank_operation_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 176105218
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }
}