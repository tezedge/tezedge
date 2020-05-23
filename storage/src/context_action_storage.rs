// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;
use std::mem;
use std::ops::Range;
use std::sync::Arc;

use getset::{CopyGetters, Getters};
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType};
use tezos_context::channel::ContextAction;
use tezos_messages::base::signature_public_key_hash::{ConversionError, SignaturePublicKeyHash};

use crate::num_from_slice;
use crate::persistent::{CommitLogSchema, CommitLogWithSchema, Decoder, Encoder, KeyValueSchema, KeyValueStoreWithSchema, Location, PersistentStorage, SchemaError};
use crate::persistent::codec::{range_from_idx_len, vec_from_slice};
use crate::persistent::commit_log::fold_consecutive_locations;
use crate::persistent::sequence::{SequenceGenerator, SequenceNumber};
use crate::StorageError;

pub type ContextActionStorageCommitLog = dyn CommitLogWithSchema<ContextActionStorage> + Sync + Send;

/// Holds all actions received from a tezos context.
/// Action is created every time a context is modified.
pub struct ContextActionStorage {
    context_primary_index: ContextActionPrimaryIndex,
    context_by_contract_index: ContextActionByContractIndex,
    clog: Arc<ContextActionStorageCommitLog>,
    generator: Arc<SequenceGenerator>,
}

impl ContextActionStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            context_primary_index: ContextActionPrimaryIndex::new(persistent_storage.kv()),
            context_by_contract_index: ContextActionByContractIndex::new(persistent_storage.kv()),
            clog: persistent_storage.clog(),
            generator: persistent_storage.seq().generator(Self::name()),
        }
    }

    #[inline]
    pub fn put_action(&mut self, block_hash: &BlockHash, action: ContextAction) -> Result<(), StorageError> {
        // generate ID
        let id = self.generator.next()?;
        let value = ContextActionRecordValue::new(action, id);
        // store into commit log
        let location = self.clog.append(&value)?;
        // populate indexes
        let primary_idx_res = self.context_primary_index.put(&ContextActionPrimaryIndexKey::new(block_hash, id), &location);
        let contract_idx_res = extract_contract_addresses(&value).iter()
            .map(|contract_address| self.context_by_contract_index.put(&ContextActionByContractIndexKey::new(contract_address, id), &location))
            .collect::<Result<(), _>>();
        primary_idx_res.and(contract_idx_res)
    }

    #[inline]
    pub fn get_by_block_hash(&self, block_hash: &BlockHash) -> Result<Vec<ContextActionRecordValue>, StorageError> {
        self.context_primary_index.get_by_block_hash(block_hash)
            .and_then(|locations| self.get_records_by_locations(&locations))
    }

    #[inline]
    pub fn get_by_contract_address(&self, contract_address: &ContractAddress, from_id: Option<SequenceNumber>, limit: usize) -> Result<Vec<ContextActionRecordValue>, StorageError> {
        self.context_by_contract_index.get_by_contract_address(contract_address, from_id, limit)
            .and_then(|locations| self.get_records_by_locations(&locations))
    }

    /// Retrieve record value from commit log or return error if value is not present.
    #[inline]
    fn get_record_by_location(&self, location: &Location) -> Result<ContextActionRecordValue, StorageError> {
        self.clog.get(location).map_err(StorageError::from).or(Err(StorageError::MissingKey))
    }

    /// Retrieve record values in batch
    fn get_records_by_locations(&self, locations: &[Location]) -> Result<Vec<ContextActionRecordValue>, StorageError> {
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

impl CommitLogSchema for ContextActionStorage {
    type Value = ContextActionRecordValue;

    #[inline]
    fn name() -> &'static str {
        "context_action_storage"
    }
}

#[derive(Getters, CopyGetters, Serialize, Deserialize)]
pub struct ContextActionRecordValue {
    #[get = "pub"]
    action: ContextAction,
    #[get_copy = "pub"]
    id: SequenceNumber,
}

impl ContextActionRecordValue {
    pub fn new(action: ContextAction, id: SequenceNumber) -> Self {
        Self { action, id }
    }

    pub fn into_action(self) -> ContextAction {
        self.action
    }
}

/// Codec for `ContextRecordValue`
impl crate::persistent::BincodeEncoded for ContextActionRecordValue { }

fn extract_contract_addresses(value: &ContextActionRecordValue) -> Vec<ContractAddress> {
    let contract_addresses = match &value.action {
        ContextAction::Set { key, .. }
        | ContextAction::Delete { key, .. }
        | ContextAction::RemoveRecursively { key, .. }
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
        .filter(|c| c.len() == ContextActionByContractIndexKey::LEN_CONTRACT_ADDRESS)
        .collect()
}

/// Extracts contract id for index from contracts keys - see [contract_id_to_contract_address]
///
/// Relevant keys for contract index should looks like:
/// 1. "data", "contracts", "index", "b5", "94", "d1", "1e", "8e", "52", "0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50", "roll_list"
/// - in this case we use exact bytes: 0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50, which conforms "contract id index" length [LEN_TOTAL] [contract_id_to_contract_address]
///
/// 2. "data", "contracts", "index", "p256", "6f", "de", "46", "af", "03", "56a0476dae4e4600172dc9309b3aa4", "balance"
/// - in this case we use exact hash to transform: p2566fde46af0356a0476dae4e4600172dc9309b3aa4, which conforms "contract id index" length [LEN_TOTAL] [contract_id_to_contract_address]
///
fn action_key_to_contract_address(key: &[String]) -> Option<ContractAddress> {
    if key.len() >= 10 && "data" == key[0] && "contracts" == key[1] && "index" == key[2] {

        // check if case 1.
        let contract_id = hex::decode(&key[9]).ok();
        let contract_id_len = contract_id.map_or(0,|cid| cid.len());
        if contract_id_len == ContextActionByContractIndexKey::LEN_CONTRACT_ADDRESS {
            return hex::decode(&key[9]).ok();
        };

        // check if case 2.
        match SignaturePublicKeyHash::from_hex_hash_and_curve(&key[4..10].join(""), &key[3].as_str()) {
            Err(_) => None,
            Ok(pubkey) => match contract_id_to_contract_address_for_index(pubkey.to_string().as_str()) {
                Err(_) => None,
                Ok(address) => Some(address)
            }
        }
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
pub struct ContextActionPrimaryIndex {
    kv: Arc<ContextActionPrimaryIndexKV>,
}

pub type ContextActionPrimaryIndexKV = dyn KeyValueStoreWithSchema<ContextActionPrimaryIndex> + Sync + Send;

impl ContextActionPrimaryIndex {
    fn new(kv: Arc<ContextActionPrimaryIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(&mut self, key: &ContextActionPrimaryIndexKey, value: &Location) -> Result<(), StorageError> {
        self.kv.put(key, value)
            .map_err(StorageError::from)
    }

    #[inline]
    fn get_by_block_hash(&self, block_hash: &BlockHash) -> Result<Vec<Location>, StorageError> {
        let key = ContextActionPrimaryIndexKey::from_block_hash_prefix(block_hash);
        self.kv.prefix_iterator(&key)?
            .map(|(_, value)| value.map_err(StorageError::from))
            .collect()
    }
}

impl KeyValueSchema for ContextActionPrimaryIndex {
    type Key = ContextActionPrimaryIndexKey;
    type Value = Location;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(ContextActionPrimaryIndexKey::LEN_BLOCK_HASH));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "context_action_storage"
    }
}

/// Key for a specific action stored in a database.
#[derive(PartialEq, Debug)]
pub struct ContextActionPrimaryIndexKey {
    block_hash: BlockHash,
    id: SequenceNumber,
}

impl ContextActionPrimaryIndexKey {
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
impl Decoder for ContextActionPrimaryIndexKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if Self::LEN_TOTAL == bytes.len() {
            let block_hash = vec_from_slice(bytes, Self::IDX_BLOCK_HASH, Self::LEN_BLOCK_HASH);
            let id = num_from_slice!(bytes, Self::IDX_ID, SequenceNumber);
            Ok(ContextActionPrimaryIndexKey { block_hash, id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

/// Encoder for `ContextPrimaryIndexKey`
///
/// * bytes layout `[block_hash(32)][id(8)]`
impl Encoder for ContextActionPrimaryIndexKey {
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
pub struct ContextActionByContractIndex {
    kv: Arc<ContextActionByContractIndexKV>,
}

pub type ContextActionByContractIndexKV = dyn KeyValueStoreWithSchema<ContextActionByContractIndex> + Sync + Send;
pub type ContractAddress = Vec<u8>;

impl ContextActionByContractIndex {
    fn new(kv: Arc<ContextActionByContractIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(&mut self, key: &ContextActionByContractIndexKey, value: &Location) -> Result<(), StorageError> {
        self.kv.put(key, value).map_err(StorageError::from)
    }

    #[inline]
    fn get_by_contract_address(&self, contract_address: &ContractAddress, from_id: Option<SequenceNumber>, limit: usize) -> Result<Vec<Location>, StorageError> {
        let iterate_from_key = from_id
            .map_or_else(|| ContextActionByContractIndexKey::from_contract_address_prefix(contract_address), |from_id| ContextActionByContractIndexKey::new(contract_address, from_id));

        self.kv.prefix_iterator(&iterate_from_key)?
            .take(limit)
            .map(|(_, value)| value.map_err(StorageError::from))
            .collect()
    }
}

impl KeyValueSchema for ContextActionByContractIndex {
    type Key = ContextActionByContractIndexKey;
    type Value = Location;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(ContextActionByContractIndexKey::LEN_CONTRACT_ADDRESS));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        cf_opts.set_comparator("reverse_id", ContextActionByContractIndexKey::reverse_id_comparator);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "context_by_contract_storage"
    }
}

/// Key for a specific action stored in a database.
#[derive(PartialEq, Debug)]
pub struct ContextActionByContractIndexKey {
    contract_address: ContractAddress,
    id: SequenceNumber,
}

impl ContextActionByContractIndexKey {
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
impl Decoder for ContextActionByContractIndexKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if Self::LEN_TOTAL == bytes.len() {
            let contract_address = bytes[Self::RANGE_CONTRACT_ADDRESS].to_vec();
            let id = num_from_slice!(bytes, Self::IDX_ID, SequenceNumber);
            Ok(ContextActionByContractIndexKey { contract_address, id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

/// Encoder for `ContextByContractIndexKey`
///
/// * bytes layout `[block_hash(32)][id(8)]`
impl Encoder for ContextActionByContractIndexKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(Self::LEN_TOTAL);
        result.extend(&self.contract_address);
        result.extend(&self.id.to_be_bytes());
        assert_eq!(result.len(), Self::LEN_TOTAL, "Result length mismatch");
        Ok(result)
    }
}

/// Dedicated function to convert contract id to contract address for indexing in storage action,
/// contract id index has specified length [LEN_TOTAL]
///
/// # Arguments
///
/// * `contract_id` - contract id (tz... or KT1...)
#[inline]
pub fn contract_id_to_contract_address_for_index(contract_id: &str) -> Result<ContractAddress, ConversionError> {
    let contract_address = {
        if contract_id.len() == 44 {
            hex::decode(contract_id)?
        } else if contract_id.len() > 3 {
            let mut contract_address = Vec::with_capacity(22);
            match &contract_id[0..3] {
                "tz1" => {
                    contract_address.extend(&[0, 0]);
                    contract_address.extend(&HashType::ContractTz1Hash.string_to_bytes(contract_id)?);
                }
                "tz2" => {
                    contract_address.extend(&[0, 1]);
                    contract_address.extend(&HashType::ContractTz2Hash.string_to_bytes(contract_id)?);
                }
                "tz3" => {
                    contract_address.extend(&[0, 2]);
                    contract_address.extend(&HashType::ContractTz3Hash.string_to_bytes(contract_id)?);
                }
                "KT1" => {
                    contract_address.push(1);
                    contract_address.extend(&HashType::ContractKt1Hash.string_to_bytes(contract_id)?);
                    contract_address.push(0);
                }
                _ => return Err(ConversionError::InvalidCurveTag { curve_tag: contract_id.to_string() })
            }
            contract_address
        } else {
            return Err(ConversionError::InvalidHash { hash: contract_id.to_string() });
        }
    };

    Ok(contract_address)
}

#[cfg(test)]
mod tests {
    use failure::Error;

    use crypto::hash::HashType;

    use super::*;

    #[test]
    fn context_record_contract_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextActionByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6548654,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextActionByContractIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextActionPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 6548654,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextActionPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn context_record_key_blank_operation_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextActionPrimaryIndexKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 176105218,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextActionPrimaryIndexKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn reverse_id_comparator_correct_order() -> Result<(), Error> {
        let a = ContextActionByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6548,
        }.encode()?;
        let b = ContextActionByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6546,
        }.encode()?;

        Ok(assert_eq!(Ordering::Less, ContextActionByContractIndexKey::reverse_id_comparator(&a, &b)))
    }

    #[test]
    fn test_contract_id_to_address() -> Result<(), failure::Error> {
        let result = contract_id_to_contract_address_for_index("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?;
        assert_eq!(result, hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?);

        let result = contract_id_to_contract_address_for_index("tz1Y68Da76MHixYhJhyU36bVh7a8C9UmtvrR")?;
        assert_eq!(result, hex::decode("00008890efbd6ca6bbd7771c116111a2eec4169e0ed8")?);

        let result = contract_id_to_contract_address_for_index("tz2LBtbMMvvguWQupgEmtfjtXy77cHgdr5TE")?;
        assert_eq!(result, hex::decode("0001823dd85cdf26e43689568436e43c20cc7c89dcb4")?);

        let result = contract_id_to_contract_address_for_index("tz3e75hU4EhDU3ukyJueh5v6UvEHzGwkg3yC")?;
        assert_eq!(result, hex::decode("0002c2fe98642abd0b7dd4bc0fc42e0a5f7c87ba56fc")?);

        let result = contract_id_to_contract_address_for_index("KT1NrjjM791v7cyo6VGy7rrzB3Dg3p1mQki3")?;
        assert_eq!(result, hex::decode("019c96e27f418b5db7c301147b3e941b41bd224fe400")?);

        Ok(())
    }

    #[test]
    fn extract_contract_address() -> Result<(), Error> {

        // ok
        let contract_address = extract_contract_addresses(&action(["data", "contracts", "index", "b5", "94", "d1", "1e", "8e", "52", "0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50", "roll_list"].to_vec()));
        assert_eq!(1, contract_address.len());
        assert_eq!("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50", hex::encode(&contract_address[0]));

        // ok
        let contract_address = extract_contract_addresses(&action(["data", "contracts", "index", "p256", "6f", "de", "46", "af", "03", "56a0476dae4e4600172dc9309b3aa4", "balance"].to_vec()));
        assert_eq!(1, contract_address.len());
        assert_eq!("00026fde46af0356a0476dae4e4600172dc9309b3aa4", hex::encode(&contract_address[0]));

        // ok
        let contract_address = extract_contract_addresses(&action(["data", "contracts", "index", "ed25519", "89", "b5", "12", "22", "97", "e589f9ba8b91f4bf74804da2fe8d4a", "frozen_balance", "0", "deposits"].to_vec()));
        assert_eq!(1, contract_address.len());
        assert_eq!("000089b5122297e589f9ba8b91f4bf74804da2fe8d4a", hex::encode(&contract_address[0]));

        // ok
        let contract_address = extract_contract_addresses(&action(["data", "contracts", "index", "AAA", "89", "b5", "12", "22", "97", "e589f9ba8b91f4bf74804da2fe8d4a", "frozen_balance", "0", "deposits"].to_vec()));
        assert_eq!(0, contract_address.len());

        Ok(())
    }

    fn to_key(key: Vec<&str>) -> Vec<String> {
        key
            .into_iter()
            .map(|k| k.to_string())
            .collect()
    }

    fn action(key: Vec<&str>) -> ContextActionRecordValue {
        let action = ContextAction::Get {
            context_hash: None,
            block_hash: None,
            operation_hash: None,
            key: to_key(key),
            value: Vec::new(),
            value_as_json: None,
            start_time: 0 as f64,
            end_time: 0 as f64,
        };
        ContextActionRecordValue::new(action, 123 as u64)
    }
}
