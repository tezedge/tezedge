// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;
use std::mem;
use std::sync::Arc;

use getset::{CopyGetters, Getters};
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType};
use tezos_context::channel::ContextAction;

use crate::num_from_slice;
use crate::persistent::{CommitLogSchema, CommitLogWithSchema, Decoder, Encoder, KeyValueSchema, KeyValueStoreWithSchema, Location, PersistentStorage, SchemaError};
use crate::persistent::codec::{vec_from_slice, range_from_idx_len};
use crate::persistent::commit_log::fold_consecutive_locations;
use crate::persistent::sequence::{SequenceGenerator, SequenceNumber};
use crate::StorageError;
use std::ops::Range;

pub type ContextStorageCommitLog = dyn CommitLogWithSchema<ContextStorage> + Sync + Send;

/// Holds all actions received from a tezos context.
/// Action is created every time a context is modified.
pub struct ContextStorage {
    context_primary_index: ContextPrimaryIndex,
    context_by_contract_index: ContextByContractIndex,
    clog: Arc<ContextStorageCommitLog>,
    generator: Arc<SequenceGenerator>,
}

impl ContextStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            context_primary_index: ContextPrimaryIndex::new(persistent_storage.kv()),
            context_by_contract_index: ContextByContractIndex::new(persistent_storage.kv()),
            clog: persistent_storage.clog(),
            generator: persistent_storage.seq().generator(Self::name()), 
        }
    }

    #[inline]
    pub fn put_action(&mut self, block_hash: &BlockHash, action: ContextAction) -> Result<(), StorageError> {
        // generate ID
        let id = self.generator.next()?;
        let value = ContextRecordValue::new(action, id);
        // store into commit log
        let location = self.clog.append(&value)?;
        // populate indexes
        let primary_idx_res = self.context_primary_index.put(&ContextPrimaryIndexKey::new(block_hash, id), &location);
        let contract_idx_res = extract_contract_addresses(&value).iter()
            .map(|contract_address| self.context_by_contract_index.put(&ContextByContractIndexKey::new(contract_address, id), &location))
            .collect::<Result<(), _>>();
        primary_idx_res.and(contract_idx_res)
    }

    #[inline]
    pub fn get_by_block_hash(&self, block_hash: &BlockHash) -> Result<Vec<ContextRecordValue>, StorageError> {
        self.context_primary_index.get_by_block_hash(block_hash)
            .and_then(|locations| self.get_records_by_locations(&locations))
    }

    #[inline]
    pub fn get_by_contract_address(&self, contract_address: &ContractAddress, from_id: Option<SequenceNumber>, limit: usize) -> Result<Vec<ContextRecordValue>, StorageError> {
        self.context_by_contract_index.get_by_contract_address(contract_address, from_id, limit)
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

#[derive(Getters, CopyGetters, Serialize, Deserialize)]
pub struct ContextRecordValue {
    #[get = "pub"]
    action: ContextAction,
    #[get_copy = "pub"]
    id: SequenceNumber,
}

impl ContextRecordValue {
    pub fn new(action: ContextAction, id: SequenceNumber) -> Self {
        Self { action, id }
    }

    pub fn into_action(self) -> ContextAction {
        self.action
    }
}

/// Codec for `ContextRecordValue`
impl crate::persistent::BincodeEncoded for ContextRecordValue { }

fn extract_contract_addresses(value: &ContextRecordValue) -> Vec<ContractAddress> {
    let contract_addresses = match &value.action {
        ContextAction::Set { key, .. }
        | ContextAction::Delete { key, .. }
        | ContextAction::RemoveRecord { key, .. }
        | ContextAction::Mem { key, .. }
        | ContextAction::DirMem { key, .. }
        | ContextAction::Get { key, .. }
        | ContextAction::Fold { key, .. } => {
            vec![action_key_to_contract_address(key)]
        }
        ContextAction::Copy { from_key, to_key, .. } => {
            vec![action_key_to_contract_address(from_key), action_key_to_contract_address(to_key)]
        }
        _ => vec![]
    };

    contract_addresses.into_iter()
        .filter_map(|c| c)
        .collect()
}

fn action_key_to_contract_address(key: &[String]) -> Option<ContractAddress> {
    if key.len() >= 10 && "data" == key[0] && "contracts" == key[1] && "index" == key[2] {
        hex::decode(&key[9]).ok()
    } else {
        None
    }
}

/// Index data as `block_hash -> location`.
///
/// Primary index is composed from:
/// * block header hash
/// * auto increment ID
///
/// This allows for fast search of context actions belonging to a block.
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
        let key = ContextPrimaryIndexKey::from_block_hash_prefix(block_hash);
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
    const LEN_ID: usize = mem::size_of::<SequenceNumber>();
    const LEN_TOTAL: usize = Self::LEN_BLOCK_HASH + Self::LEN_ID;

    const IDX_BLOCK_HASH: usize = 0;
    const IDX_ID: usize = Self::IDX_BLOCK_HASH + Self::LEN_BLOCK_HASH;

    pub fn new(block_hash: &BlockHash, id: SequenceNumber) -> Self {
        Self {
            block_hash: block_hash.clone(),
            id,
        }
    }

    /// This is useful only when using prefix iterator to retrieve
    /// actions belonging to the same block.
    fn from_block_hash_prefix(block_hash: &BlockHash) -> Self {
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
        if Self::LEN_TOTAL == bytes.len() {
            let block_hash = vec_from_slice(bytes, Self::IDX_BLOCK_HASH, Self::LEN_BLOCK_HASH);
            let id = num_from_slice!(bytes, Self::IDX_ID, SequenceNumber);
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
        let mut result = Vec::with_capacity(Self::LEN_TOTAL);
        result.extend(&self.block_hash);
        result.extend(&self.id.to_be_bytes());
        assert_eq!(result.len(), Self::LEN_TOTAL, "Result length mismatch");
        Ok(result)
    }
}


/// Index data as `contract_address -> location`.
///
/// Index is composed from:
/// * contract address
/// * auto increment ID
///
/// This allows for fast search of context actions belonging to a contract.
pub struct ContextByContractIndex {
    kv: Arc<ContextByContractIndexKV>,
}

pub type ContextByContractIndexKV = dyn KeyValueStoreWithSchema<ContextByContractIndex> + Sync + Send;
pub type ContractAddress = Vec<u8>;

impl ContextByContractIndex {
    fn new(kv: Arc<ContextByContractIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(&mut self, key: &ContextByContractIndexKey, value: &Location) -> Result<(), StorageError> {
        self.kv.put(key, value).map_err(StorageError::from)
    }

    #[inline]
    fn get_by_contract_address(&self, contract_address: &ContractAddress, from_id: Option<SequenceNumber>, limit: usize) -> Result<Vec<Location>, StorageError> {
        let iterate_from_key = from_id
            .map_or_else(|| ContextByContractIndexKey::from_contract_address_prefix(contract_address), |from_id| ContextByContractIndexKey::new(contract_address, from_id));

        self.kv.prefix_iterator(&iterate_from_key)?
            .take(limit)
            .map(|(_, value)| value.map_err(StorageError::from))
            .collect()
    }
}

impl KeyValueSchema for ContextByContractIndex {
    type Key = ContextByContractIndexKey;
    type Value = Location;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(ContextByContractIndexKey::LEN_CONTRACT_ADDRESS));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        cf_opts.set_comparator("reverse_id", ContextByContractIndexKey::reverse_id_comparator);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "context_by_contract_storage"
    }
}

/// Key for a specific action stored in a database.
#[derive(PartialEq, Debug)]
pub struct ContextByContractIndexKey {
    contract_address: ContractAddress,
    id: SequenceNumber,
}

impl ContextByContractIndexKey {
    const LEN_CONTRACT_ADDRESS: usize = 22;
    const LEN_ID: usize = mem::size_of::<SequenceNumber>();
    const LEN_TOTAL: usize = Self::LEN_CONTRACT_ADDRESS + Self::LEN_ID;

    const IDX_CONTRACT_ADDRESS: usize = 0;
    const IDX_ID: usize = Self::IDX_CONTRACT_ADDRESS + Self::LEN_CONTRACT_ADDRESS;

    const RANGE_CONTRACT_ADDRESS: Range<usize> = range_from_idx_len(Self::IDX_CONTRACT_ADDRESS, Self::LEN_CONTRACT_ADDRESS);
    const RANGE_ID: Range<usize> = Self::IDX_ID..Self::IDX_ID + Self::LEN_ID;

    pub fn new(contract_address: &[u8], id: SequenceNumber) -> Self {
        Self {
            contract_address: contract_address.to_vec(),
            id,
        }
    }

    /// This is useful only when using prefix iterator to retrieve
    /// actions belonging to the same block.
    fn from_contract_address_prefix(contract_address: &[u8]) -> Self {
        Self {
            contract_address: contract_address.to_vec(),
            id: std::u64::MAX,
        }
    }

    /// Comparator that sorts records in a descending order.
    fn reverse_id_comparator(a: &[u8], b: &[u8]) -> Ordering {
        assert_eq!(a.len(), b.len(), "Cannot compare keys of mismatching length");
        assert_eq!(a.len(), Self::LEN_TOTAL, "Key is expected to have exactly {} bytes", Self::LEN_TOTAL);

        for idx in Self::RANGE_CONTRACT_ADDRESS {
            match a[idx].cmp(&b[idx]) {
                Ordering::Greater => return Ordering::Greater,
                Ordering::Less => return Ordering::Less,
                Ordering::Equal => ()
            }
        }
        // order ID in reverse
        for idx in Self::RANGE_ID {
            match a[idx].cmp(&b[idx]) {
                Ordering::Greater => return Ordering::Less,
                Ordering::Less => return Ordering::Greater,
                Ordering::Equal => ()
            }
        }

        Ordering::Equal
    }
}

/// Decoder for `ContextByContractIndexKey`
///
/// * bytes layout `[block_hash(32)][id(8)]`
impl Decoder for ContextByContractIndexKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if Self::LEN_TOTAL == bytes.len() {
            let contract_address = bytes[Self::RANGE_CONTRACT_ADDRESS].to_vec();
            let id = num_from_slice!(bytes, Self::IDX_ID, SequenceNumber);
            Ok(ContextByContractIndexKey { contract_address, id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

/// Encoder for `ContextByContractIndexKey`
///
/// * bytes layout `[block_hash(32)][id(8)]`
impl Encoder for ContextByContractIndexKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(Self::LEN_TOTAL);
        result.extend(&self.contract_address);
        result.extend(&self.id.to_be_bytes());
        assert_eq!(result.len(), Self::LEN_TOTAL, "Result length mismatch");
        Ok(result)
    }
}


#[cfg(test)]
mod tests {
    use failure::Error;

    use crypto::hash::HashType;

    use super::*;

    #[test]
    fn context_record_contract_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6548654,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextByContractIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 6548654,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_key_blank_operation_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 176105218,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn reverse_id_comparator_correct_order() -> Result<(), Error> {
        let a = ContextByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6548,
        }.encode()?;
        let b = ContextByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6546,
        }.encode()?;

        Ok(assert_eq!(Ordering::Less, ContextByContractIndexKey::reverse_id_comparator(&a, &b)))
    }
}