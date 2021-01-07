// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;
use std::mem;
use std::ops::Range;
use std::str::FromStr;
use std::sync::Arc;

use failure::Fail;
use rocksdb::{Cache, ColumnFamilyDescriptor, SliceTransform};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType};
use tezos_context::channel::ContextAction;
use tezos_messages::base::signature_public_key_hash::{ConversionError, SignaturePublicKeyHash};

use crate::persistent::codec::{range_from_idx_len, vec_from_slice};
use crate::persistent::sequence::{SequenceGenerator, SequenceNumber};
use crate::persistent::{
    default_table_options, BincodeEncoded, Decoder, Encoder, KeyValueSchema,
    KeyValueStoreWithSchema, PersistentStorage, SchemaError,
};
use crate::StorageError;
use crate::{num_from_slice, persistent::StorageType};

pub enum ContextHashType {
    Block,
    Contract,
}

pub struct ContextActionFilters {
    pub hash: (ContextHashType, Vec<u8>),
    pub action_type: Option<Vec<ContextActionType>>,
}

impl ContextActionFilters {
    pub fn with_block_hash(block_hash: Vec<u8>) -> Self {
        Self {
            hash: (ContextHashType::Block, block_hash),
            action_type: None,
        }
    }

    pub fn with_contract_id(contract_hash: Vec<u8>) -> Self {
        Self {
            hash: (ContextHashType::Contract, contract_hash),
            action_type: None,
        }
    }

    pub fn with_action_types(mut self, action_type: Vec<ContextActionType>) -> Self {
        self.action_type = Some(action_type);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.action_type.is_none()
    }
}

pub type ContextActionStorageKV = dyn KeyValueStoreWithSchema<ContextActionStorage> + Sync + Send;

/// Holds all actions received from a tezos context.
/// Action is created every time a context is modified.
pub struct ContextActionStorage {
    context_by_block_index: ContextActionByBlockHashIndex,
    context_by_contract_index: ContextActionByContractIndex,
    context_by_type_index: ContextActionByTypeIndex,
    kv: Arc<ContextActionStorageKV>,
    generator: Arc<SequenceGenerator>,
}

impl ContextActionStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        let storage = persistent_storage.kv(StorageType::ContextAction);
        Self {
            kv: persistent_storage.kv(StorageType::ContextAction),
            generator: persistent_storage.seq().generator(Self::name()),
            context_by_block_index: ContextActionByBlockHashIndex::new(storage.clone()),
            context_by_contract_index: ContextActionByContractIndex::new(storage.clone()),
            context_by_type_index: ContextActionByTypeIndex::new(storage),
        }
    }

    #[inline]
    pub fn put_action(
        &mut self,
        block_hash: &BlockHash,
        action: ContextAction,
    ) -> Result<(), StorageError> {
        // generate ID
        let id = self.generator.next()?;
        let action = ContextActionRecordValue::new(action, id);
        // Store action
        self.kv.put(&id, &action)?;
        // Populate indexes
        self.context_by_block_index
            .put(&ContextActionByBlockHashKey::new(block_hash, id))?;

        if let Some(action_type) = ContextActionType::extract_type(action.action()) {
            self.context_by_type_index
                .put(&ContextActionByTypeIndexKey::new(action_type, id))?;
        }

        extract_contract_addresses(&action)
            .iter()
            .map(|contract_address| {
                self.context_by_contract_index
                    .put(&ContextActionByContractIndexKey::new(contract_address, id))
            })
            .collect::<Result<(), _>>()
    }

    #[inline]
    pub fn load_cursor(
        &self,
        cursor_id: Option<SequenceNumber>,
        limit: Option<usize>,
        cursor_filters: ContextActionFilters,
    ) -> Result<Vec<ContextActionRecordValue>, StorageError> {
        let (addr_type, hash) = cursor_filters.hash;
        if let ContextHashType::Block = addr_type {
            let base_iterator = self
                .context_by_block_index
                .get_by_block_hash_iterator(&hash, cursor_id)?;
            if let Some(action_type) = cursor_filters.action_type {
                let type_iterator = self
                    .context_by_type_index
                    .get_by_action_types_iterator(&action_type, None)?;
                let iterators: Vec<Box<dyn Iterator<Item = SequenceNumber>>> =
                    vec![Box::new(base_iterator), Box::new(type_iterator)];
                self.load_indexes(
                    sorted_intersect::sorted_intersect(iterators, limit.unwrap_or(std::usize::MAX))
                        .into_iter(),
                )
            } else if let Some(limit) = limit {
                self.load_indexes(base_iterator.take(limit))
            } else {
                self.load_indexes(base_iterator)
            }
        } else {
            let mut base_iterator = self
                .context_by_contract_index
                .get_by_contract_address_iterator(&hash, cursor_id)?
                .peekable();
            if let Some(action_type) = cursor_filters.action_type {
                if let Some(index) = base_iterator.peek() {
                    let type_iterator = self
                        .context_by_type_index
                        .get_by_action_types_iterator(&action_type, Some(*index))?;
                    let iterators: Vec<Box<dyn Iterator<Item = SequenceNumber>>> =
                        vec![Box::new(base_iterator), Box::new(type_iterator)];
                    self.load_indexes(
                        sorted_intersect::sorted_intersect(
                            iterators,
                            limit.unwrap_or(std::usize::MAX),
                        )
                        .into_iter(),
                    )
                } else {
                    Ok(Default::default())
                }
            } else if let Some(limit) = limit {
                self.load_indexes(base_iterator.take(limit))
            } else {
                self.load_indexes(base_iterator)
            }
        }
    }

    #[inline]
    pub fn get_by_block_hash(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Vec<ContextActionRecordValue>, StorageError> {
        self.context_by_block_index
            .get_by_block_hash(block_hash)
            .and_then(|idx| self.load_indexes(idx.into_iter()))
    }

    #[inline]
    pub fn get_by_contract_address(
        &self,
        contract_address: &ContractAddress,
        from_id: Option<SequenceNumber>,
        limit: usize,
    ) -> Result<Vec<ContextActionRecordValue>, StorageError> {
        self.context_by_contract_index
            .get_by_contract_address(contract_address, from_id, limit)
            .and_then(|idx| self.load_indexes(idx.into_iter()))
    }

    fn load_indexes<'a, Idx: Iterator<Item = u64> + 'a>(
        &'a self,
        indexes: Idx,
    ) -> Result<Vec<ContextActionRecordValue>, StorageError> {
        Ok(indexes
            .filter_map(|id| self.kv.get(&id).ok().flatten())
            .collect())
    }
}

impl KeyValueSchema for ContextActionStorage {
    type Key = SequenceNumber;
    type Value = ContextActionRecordValue;

    #[inline]
    fn name() -> &'static str {
        "context_action_storage"
    }
}

#[derive(Serialize, Deserialize)]
pub struct ContextActionRecordValue {
    pub action: ContextAction,
    pub id: SequenceNumber,
}

impl ContextActionRecordValue {
    pub fn new(action: ContextAction, id: SequenceNumber) -> Self {
        Self { action, id }
    }

    pub fn id(&self) -> SequenceNumber {
        self.id
    }

    pub fn action(&self) -> &ContextAction {
        &self.action
    }

    pub fn into_action(self) -> ContextAction {
        self.action
    }
}

impl BincodeEncoded for ContextActionRecordValue {}

#[derive(Serialize, Deserialize)]
pub struct ContextActionJson {
    #[serde(flatten)]
    pub action: ContextAction,
    pub id: SequenceNumber,
}

impl From<ContextActionRecordValue> for ContextActionJson {
    fn from(rv: ContextActionRecordValue) -> Self {
        Self {
            action: rv.action,
            id: rv.id,
        }
    }
}

fn extract_contract_addresses(value: &ContextActionRecordValue) -> Vec<ContractAddress> {
    let contract_addresses = match &value.action {
        ContextAction::Set { key, .. }
        | ContextAction::Delete { key, .. }
        | ContextAction::RemoveRecursively { key, .. }
        | ContextAction::Mem { key, .. }
        | ContextAction::DirMem { key, .. }
        | ContextAction::Get { key, .. }
        | ContextAction::Fold { key, .. } => vec![action_key_to_contract_address(key)],
        ContextAction::Copy {
            from_key, to_key, ..
        } => vec![
            action_key_to_contract_address(from_key),
            action_key_to_contract_address(to_key),
        ],
        _ => vec![],
    };

    contract_addresses
        .into_iter()
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
        let contract_id_len = contract_id.map_or(0, |cid| cid.len());
        if contract_id_len == ContextActionByContractIndexKey::LEN_CONTRACT_ADDRESS {
            return hex::decode(&key[9]).ok();
        };

        // check if case 2.
        match SignaturePublicKeyHash::from_hex_hash_and_curve(
            &key[4..10].join(""),
            &key[3].as_str(),
        ) {
            Err(_) => None,
            Ok(pubkey) => match contract_id_to_contract_address_for_index(
                pubkey.to_string_representation().as_str(),
            ) {
                Err(_) => None,
                Ok(address) => Some(address),
            },
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
pub struct ContextActionByBlockHashIndex {
    kv: Arc<ContextActionBlockHashIndexKV>,
}

pub type ContextActionBlockHashIndexKV =
    dyn KeyValueStoreWithSchema<ContextActionByBlockHashIndex> + Sync + Send;

impl ContextActionByBlockHashIndex {
    fn new(kv: Arc<ContextActionBlockHashIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(&mut self, key: &ContextActionByBlockHashKey) -> Result<(), StorageError> {
        self.kv.put(key, &()).map_err(StorageError::from)
    }

    #[inline]
    fn get_by_block_hash(
        &self,
        block_hash: &BlockHash,
    ) -> Result<Vec<SequenceNumber>, StorageError> {
        Ok(self.get_by_block_hash_iterator(block_hash, None)?.collect())
    }

    #[inline]
    fn get_by_block_hash_iterator<'a>(
        &'a self,
        block_hash: &BlockHash,
        cursor_id: Option<SequenceNumber>,
    ) -> Result<impl Iterator<Item = SequenceNumber> + 'a, StorageError> {
        let key = cursor_id.map_or_else(
            || ContextActionByBlockHashKey::from_block_hash_prefix(block_hash),
            |cursor_id| ContextActionByBlockHashKey::new(block_hash, cursor_id),
        );
        Ok(self
            .kv
            .prefix_iterator(&key)?
            .filter_map(|(key, _)| key.map(|k| k.id).ok()))
    }
}

impl KeyValueSchema for ContextActionByBlockHashIndex {
    type Key = ContextActionByBlockHashKey;
    type Value = ();

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options(cache);
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(
            ContextActionByBlockHashKey::LEN_BLOCK_HASH,
        ));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "context_action_block_hash_index"
    }
}

/// Key for a specific action stored in a database.
#[derive(PartialEq, Debug)]
pub struct ContextActionByBlockHashKey {
    block_hash: BlockHash,
    id: SequenceNumber,
}

impl ContextActionByBlockHashKey {
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
impl Decoder for ContextActionByBlockHashKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if Self::LEN_TOTAL == bytes.len() {
            let block_hash = vec_from_slice(bytes, Self::IDX_BLOCK_HASH, Self::LEN_BLOCK_HASH);
            let id = num_from_slice!(bytes, Self::IDX_ID, SequenceNumber);
            Ok(Self { block_hash, id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

/// Encoder for `ContextPrimaryIndexKey`
///
/// * bytes layout `[block_hash(32)][id(8)]`
impl Encoder for ContextActionByBlockHashKey {
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

pub type ContextActionByContractIndexKV =
    dyn KeyValueStoreWithSchema<ContextActionByContractIndex> + Sync + Send;
pub type ContractAddress = Vec<u8>;

impl ContextActionByContractIndex {
    fn new(kv: Arc<ContextActionByContractIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(&mut self, key: &ContextActionByContractIndexKey) -> Result<(), StorageError> {
        self.kv.put(key, &()).map_err(StorageError::from)
    }

    #[inline]
    fn get_by_contract_address(
        &self,
        contract_address: &ContractAddress,
        from_id: Option<SequenceNumber>,
        limit: usize,
    ) -> Result<Vec<SequenceNumber>, StorageError> {
        Ok(self
            .get_by_contract_address_iterator(contract_address, from_id)?
            .take(limit)
            .collect())
    }

    #[inline]
    fn get_by_contract_address_iterator<'a>(
        &'a self,
        contract_address: &ContractAddress,
        cursor_id: Option<SequenceNumber>,
    ) -> Result<impl Iterator<Item = SequenceNumber> + 'a, StorageError> {
        let iterate_from_key = cursor_id.map_or_else(
            || ContextActionByContractIndexKey::from_contract_address_prefix(contract_address),
            |cursor_id| ContextActionByContractIndexKey::new(contract_address, cursor_id),
        );

        Ok(self
            .kv
            .prefix_iterator(&iterate_from_key)?
            .filter_map(|(key, _)| key.map(|index_key| index_key.id).ok()))
    }
}

impl KeyValueSchema for ContextActionByContractIndex {
    type Key = ContextActionByContractIndexKey;
    type Value = ();

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options(cache);
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(
            ContextActionByContractIndexKey::LEN_CONTRACT_ADDRESS,
        ));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        // cf_opts.set_comparator("reverse_id", ContextActionByContractIndexKey::reverse_id_comparator);
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

#[allow(dead_code)]
impl ContextActionByContractIndexKey {
    const LEN_CONTRACT_ADDRESS: usize = 22;
    const LEN_ID: usize = mem::size_of::<SequenceNumber>();
    const LEN_TOTAL: usize = Self::LEN_CONTRACT_ADDRESS + Self::LEN_ID;

    const IDX_CONTRACT_ADDRESS: usize = 0;
    const IDX_ID: usize = Self::IDX_CONTRACT_ADDRESS + Self::LEN_CONTRACT_ADDRESS;

    const RANGE_CONTRACT_ADDRESS: Range<usize> =
        range_from_idx_len(Self::IDX_CONTRACT_ADDRESS, Self::LEN_CONTRACT_ADDRESS);
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
            id: 0,
        }
    }

    /// Comparator that sorts records in a descending order.
    #[allow(dead_code)]
    fn reverse_id_comparator(a: &[u8], b: &[u8]) -> Ordering {
        assert_eq!(
            a.len(),
            b.len(),
            "Cannot compare keys of mismatching length"
        );
        assert_eq!(
            a.len(),
            Self::LEN_TOTAL,
            "Key is expected to have exactly {} bytes",
            Self::LEN_TOTAL
        );

        for idx in Self::RANGE_CONTRACT_ADDRESS {
            match a[idx].cmp(&b[idx]) {
                Ordering::Greater => return Ordering::Greater,
                Ordering::Less => return Ordering::Less,
                Ordering::Equal => (),
            }
        }
        // order ID in reverse
        for idx in Self::RANGE_ID {
            match a[idx].cmp(&b[idx]) {
                Ordering::Greater => return Ordering::Less,
                Ordering::Less => return Ordering::Greater,
                Ordering::Equal => (),
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
            Ok(ContextActionByContractIndexKey {
                contract_address,
                id,
            })
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
pub fn contract_id_to_contract_address_for_index(
    contract_id: &str,
) -> Result<ContractAddress, ConversionError> {
    let contract_address = {
        if contract_id.len() == 44 {
            hex::decode(contract_id)?
        } else if contract_id.len() > 3 {
            let mut contract_address = Vec::with_capacity(22);
            match &contract_id[0..3] {
                "tz1" => {
                    contract_address.extend(&[0, 0]);
                    contract_address
                        .extend(&HashType::ContractTz1Hash.b58check_to_hash(contract_id)?);
                }
                "tz2" => {
                    contract_address.extend(&[0, 1]);
                    contract_address
                        .extend(&HashType::ContractTz2Hash.b58check_to_hash(contract_id)?);
                }
                "tz3" => {
                    contract_address.extend(&[0, 2]);
                    contract_address
                        .extend(&HashType::ContractTz3Hash.b58check_to_hash(contract_id)?);
                }
                "KT1" => {
                    contract_address.push(1);
                    contract_address
                        .extend(&HashType::ContractKt1Hash.b58check_to_hash(contract_id)?);
                    contract_address.push(0);
                }
                _ => {
                    return Err(ConversionError::InvalidCurveTag {
                        curve_tag: contract_id.to_string(),
                    })
                }
            }
            contract_address
        } else {
            return Err(ConversionError::InvalidHash {
                hash: contract_id.to_string(),
            });
        }
    };

    Ok(contract_address)
}

/// Type index
pub struct ContextActionByTypeIndex {
    kv: Arc<ContextActionByTypeIndexKV>,
}

pub type ContextActionByTypeIndexKV =
    dyn KeyValueStoreWithSchema<ContextActionByTypeIndex> + Sync + Send;

impl ContextActionByTypeIndex {
    fn new(kv: Arc<ContextActionByTypeIndexKV>) -> Self {
        Self { kv }
    }

    #[inline]
    fn put(&self, key: &ContextActionByTypeIndexKey) -> Result<(), StorageError> {
        self.kv.put(key, &()).map_err(StorageError::from)
    }

    #[inline]
    fn get_by_action_type_iterator(
        &self,
        action_type: ContextActionType,
        cursor_id: Option<SequenceNumber>,
    ) -> Result<impl Iterator<Item = u64> + '_, StorageError> {
        let iterate_from_key = cursor_id.map_or_else(
            || ContextActionByTypeIndexKey::from_action_type_prefix(action_type),
            |cursor_id| ContextActionByTypeIndexKey::new(action_type, cursor_id),
        );

        Ok(self
            .kv
            .prefix_iterator(&iterate_from_key)?
            .filter_map(|(key, _)| key.map(|id_key| id_key.id).ok()))
    }

    #[inline]
    fn get_by_action_types_iterator<'a>(
        &'a self,
        action_types: &'a [ContextActionType],
        cursor_id: Option<SequenceNumber>,
    ) -> Result<impl Iterator<Item = u64> + 'a, StorageError> {
        use itertools::kmerge_by;
        let mut ret = Vec::with_capacity(action_types.len());
        for action_type in action_types {
            ret.push(self.get_by_action_type_iterator(*action_type, cursor_id)?);
        }
        let cmp = |a: &u64, b: &u64| a > b;
        Ok(kmerge_by(ret.into_iter(), cmp))
    }
}

impl KeyValueSchema for ContextActionByTypeIndex {
    type Key = ContextActionByTypeIndexKey;
    type Value = ();

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options(cache);
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(mem::size_of::<
            ContextActionType,
        >()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        // cf_opts.set_comparator("reverse_id", ContextActionByTypeIndexKey::reverse_id_comparator);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    fn name() -> &'static str {
        "context_by_type_storage"
    }
}

#[derive(PartialEq, Debug)]
pub struct ContextActionByTypeIndexKey {
    pub action_type: ContextActionType,
    pub id: SequenceNumber,
}

impl ContextActionByTypeIndexKey {
    const LEN_TYPE: usize = mem::size_of::<ContextActionType>();
    const LEN_ID: usize = mem::size_of::<SequenceNumber>();
    const LEN_TOTAL: usize = Self::LEN_TYPE + Self::LEN_ID;

    pub fn new(action_type: ContextActionType, id: SequenceNumber) -> Self {
        Self { action_type, id }
    }

    fn from_action_type_prefix(action_type: ContextActionType) -> Self {
        Self::new(action_type, std::u64::MIN)
    }

    #[allow(dead_code)]
    fn reverse_id_comparator(a: &[u8], b: &[u8]) -> Ordering {
        assert_eq!(
            a.len(),
            b.len(),
            "Cannot compare keys of mismatching length"
        );
        assert_eq!(
            a.len(),
            Self::LEN_TOTAL,
            "Key is expected to have exactly {} bytes",
            Self::LEN_TOTAL
        );

        let range = 0..Self::LEN_TYPE;
        for (a, b) in a[range.clone()].iter().zip(&b[range]) {
            match a.cmp(b) {
                Ordering::Greater => return Ordering::Less,
                Ordering::Less => return Ordering::Greater,
                Ordering::Equal => continue,
            }
        }

        let range = Self::LEN_TYPE..;
        for (a, b) in a[range.clone()].iter().zip(&b[range]) {
            match a.cmp(b) {
                Ordering::Greater => return Ordering::Less,
                Ordering::Less => return Ordering::Greater,
                Ordering::Equal => continue,
            }
        }

        Ordering::Equal
    }
}

/// Decoder for `ContextActionByTypeIndexKey`
///
/// * bytes layout `[action_type(2)][id(8)]`
impl Decoder for ContextActionByTypeIndexKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if Self::LEN_TOTAL == bytes.len() {
            let action_type: u16 = num_from_slice!(bytes, 0, u16);
            let action_type = ContextActionType::from_u16(action_type);
            let id = num_from_slice!(bytes, 2, SequenceNumber);
            if let Some(action_type) = action_type {
                Ok(Self { action_type, id })
            } else {
                Err(SchemaError::DecodeError)
            }
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

impl Encoder for ContextActionByTypeIndexKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut result = Vec::with_capacity(Self::LEN_TOTAL);
        result.extend_from_slice(&(self.action_type as u16).to_be_bytes());
        result.extend_from_slice(&self.id.to_be_bytes());
        assert_eq!(result.len(), Self::LEN_TOTAL, "Result length mismatch");
        Ok(result)
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub enum ContextActionType {
    Set = 0x1 << 0,
    Delete = 0x1 << 1,
    RemoveRecursively = 0x1 << 2,
    Copy = 0x1 << 3,
    Checkout = 0x1 << 4,
    Commit = 0x1 << 5,
    Mem = 0x1 << 6,
    DirMem = 0x1 << 7,
    Get = 0x1 << 8,
    Fold = 0x1 << 9,
}

impl ContextActionType {
    pub fn extract_type(value: &ContextAction) -> Option<Self> {
        match value {
            ContextAction::Set { .. } => Some(Self::Set),
            ContextAction::Delete { .. } => Some(Self::Delete),
            ContextAction::RemoveRecursively { .. } => Some(Self::RemoveRecursively),
            ContextAction::Copy { .. } => Some(Self::Copy),
            ContextAction::Checkout { .. } => Some(Self::Checkout),
            ContextAction::Commit { .. } => Some(Self::Commit),
            ContextAction::Mem { .. } => Some(Self::Mem),
            ContextAction::DirMem { .. } => Some(Self::DirMem),
            ContextAction::Get { .. } => Some(Self::Get),
            ContextAction::Fold { .. } => Some(Self::Fold),
            _ => None,
        }
    }

    pub fn from_u16(value: u16) -> Option<Self> {
        match value {
            x if x == Self::Set as u16 => Some(Self::Set),
            x if x == Self::Delete as u16 => Some(Self::Delete),
            x if x == Self::RemoveRecursively as u16 => Some(Self::RemoveRecursively),
            x if x == Self::Copy as u16 => Some(Self::Copy),
            x if x == Self::Checkout as u16 => Some(Self::Checkout),
            x if x == Self::Commit as u16 => Some(Self::Commit),
            x if x == Self::Mem as u16 => Some(Self::Mem),
            x if x == Self::DirMem as u16 => Some(Self::DirMem),
            x if x == Self::Get as u16 => Some(Self::Get),
            x if x == Self::Fold as u16 => Some(Self::Fold),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Fail)]
#[fail(display = "invalid context action type: {}", _0)]
pub struct ParseContextActionType(String);

impl FromStr for ContextActionType {
    type Err = ParseContextActionType;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Set" => Ok(Self::Set),
            "Delete" => Ok(Self::Delete),
            "RemoveRecursively" => Ok(Self::RemoveRecursively),
            "Copy" => Ok(Self::Copy),
            "Checkout" => Ok(Self::Checkout),
            "Commit" => Ok(Self::Commit),
            "Mem" => Ok(Self::Mem),
            "DirMem" => Ok(Self::DirMem),
            "Get" => Ok(Self::Get),
            "Fold" => Ok(Self::Fold),
            x => Err(ParseContextActionType(x.to_string())),
        }
    }
}

pub mod sorted_intersect {
    use std::cmp::Ordering;

    pub fn sorted_intersect<I>(mut iters: Vec<I>, limit: usize) -> Vec<I::Item>
    where
        I: Iterator,
        I::Item: Ord,
    {
        let mut ret = Default::default();
        if iters.is_empty() {
            return ret;
        } else if iters.len() == 1 {
            let iter = iters.iter_mut().next().unwrap();
            ret.extend(iter.take(limit));
            return ret;
        }
        let mut heap = Vec::with_capacity(iters.len());
        // Fill the heap with values
        if !fill_heap(iters.iter_mut(), &mut heap) {
            // Hit an exhausted iterator, finish
            return ret;
        }

        while ret.len() < limit {
            if is_hit(&heap) {
                // We hit intersected item
                if let Some((item, _)) = heap.pop() {
                    // Push it into the intersect values
                    ret.push(item);
                    // Clear the rest of the heap
                    heap.clear();
                    // Build a new heap from new values
                    if !fill_heap(iters.iter_mut(), &mut heap) {
                        // Hit an exhausted iterator, finish
                        return ret;
                    }
                } else {
                    // Hit an exhausted iterator, finish
                    return ret;
                }
            } else {
                // Remove max element from the heap
                if let Some((_, iter_num)) = heap.pop() {
                    if let Some(item) = iters[iter_num].next() {
                        // Insert replacement from the corresponding iterator to heap
                        heap.push((item, iter_num));
                        heapify(&mut heap);
                    } else {
                        // Hit an exhausted iterator, finish
                        return ret;
                    }
                } else {
                    // Hit an exhausted iterator, finish
                    return ret;
                }
            }
        }

        ret
    }

    fn heapify<Item: Ord>(heap: &mut Vec<(Item, usize)>) {
        heap.sort_by(|(a, _), (b, _)| b.cmp(a));
    }

    fn fill_heap<
        'a,
        Item: Ord,
        Inner: 'a + Iterator<Item = Item>,
        Outer: Iterator<Item = &'a mut Inner>,
    >(
        iters: Outer,
        heap: &mut Vec<(Inner::Item, usize)>,
    ) -> bool {
        for (i, iter) in iters.enumerate() {
            let value = iter.next();
            if let Some(value) = value {
                heap.push((value, i))
            } else {
                return false;
            }
        }
        heapify(heap);
        true
    }

    fn is_hit<Item: Ord>(heap: &Vec<(Item, usize)>) -> bool {
        let value = heap.iter().next().map(|(value, _)| {
            heap.iter().fold((value, true), |(a, eq), (b, _)| {
                (b, eq & (a.cmp(b) == Ordering::Equal))
            })
        });

        matches!(value, Some((_, true)))
    }
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
        assert_eq!(expected, decoded);
        Ok(())
    }

    #[test]
    fn context_record_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextActionByBlockHashKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 6548654,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextActionByBlockHashKey::decode(&encoded_bytes)?;
        assert_eq!(expected, decoded);
        Ok(())
    }

    #[test]
    fn context_record_key_blank_operation_encoded_equals_decoded() -> Result<(), Error> {
        let expected = ContextActionByBlockHashKey {
            block_hash: vec![43; HashType::BlockHash.size()],
            id: 176105218,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = ContextActionByBlockHashKey::decode(&encoded_bytes)?;
        assert_eq!(expected, decoded);
        Ok(())
    }

    #[test]
    fn reverse_id_comparator_correct_order() -> Result<(), Error> {
        let a = ContextActionByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6548,
        }
        .encode()?;
        let b = ContextActionByContractIndexKey {
            contract_address: hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?,
            id: 6546,
        }
        .encode()?;

        assert_eq!(
            Ordering::Less,
            ContextActionByContractIndexKey::reverse_id_comparator(&a, &b)
        );
        Ok(())
    }

    #[test]
    fn test_contract_id_to_address() -> Result<(), failure::Error> {
        let result = contract_id_to_contract_address_for_index(
            "0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50",
        )?;
        assert_eq!(
            result,
            hex::decode("0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50")?
        );

        let result =
            contract_id_to_contract_address_for_index("tz1Y68Da76MHixYhJhyU36bVh7a8C9UmtvrR")?;
        assert_eq!(
            result,
            hex::decode("00008890efbd6ca6bbd7771c116111a2eec4169e0ed8")?
        );

        let result =
            contract_id_to_contract_address_for_index("tz2LBtbMMvvguWQupgEmtfjtXy77cHgdr5TE")?;
        assert_eq!(
            result,
            hex::decode("0001823dd85cdf26e43689568436e43c20cc7c89dcb4")?
        );

        let result =
            contract_id_to_contract_address_for_index("tz3e75hU4EhDU3ukyJueh5v6UvEHzGwkg3yC")?;
        assert_eq!(
            result,
            hex::decode("0002c2fe98642abd0b7dd4bc0fc42e0a5f7c87ba56fc")?
        );

        let result =
            contract_id_to_contract_address_for_index("KT1NrjjM791v7cyo6VGy7rrzB3Dg3p1mQki3")?;
        assert_eq!(
            result,
            hex::decode("019c96e27f418b5db7c301147b3e941b41bd224fe400")?
        );

        Ok(())
    }

    #[test]
    fn extract_contract_address() -> Result<(), Error> {
        // ok
        let contract_address = extract_contract_addresses(&action(
            [
                "data",
                "contracts",
                "index",
                "b5",
                "94",
                "d1",
                "1e",
                "8e",
                "52",
                "0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50",
                "roll_list",
            ]
            .to_vec(),
        ));
        assert_eq!(1, contract_address.len());
        assert_eq!(
            "0000cf49f66b9ea137e11818f2a78b4b6fc9895b4e50",
            hex::encode(&contract_address[0])
        );

        // ok
        let contract_address = extract_contract_addresses(&action(
            [
                "data",
                "contracts",
                "index",
                "p256",
                "6f",
                "de",
                "46",
                "af",
                "03",
                "56a0476dae4e4600172dc9309b3aa4",
                "balance",
            ]
            .to_vec(),
        ));
        assert_eq!(1, contract_address.len());
        assert_eq!(
            "00026fde46af0356a0476dae4e4600172dc9309b3aa4",
            hex::encode(&contract_address[0])
        );

        // ok
        let contract_address = extract_contract_addresses(&action(
            [
                "data",
                "contracts",
                "index",
                "ed25519",
                "89",
                "b5",
                "12",
                "22",
                "97",
                "e589f9ba8b91f4bf74804da2fe8d4a",
                "frozen_balance",
                "0",
                "deposits",
            ]
            .to_vec(),
        ));
        assert_eq!(1, contract_address.len());
        assert_eq!(
            "000089b5122297e589f9ba8b91f4bf74804da2fe8d4a",
            hex::encode(&contract_address[0])
        );

        // ok
        let contract_address = extract_contract_addresses(&action(
            [
                "data",
                "contracts",
                "index",
                "AAA",
                "89",
                "b5",
                "12",
                "22",
                "97",
                "e589f9ba8b91f4bf74804da2fe8d4a",
                "frozen_balance",
                "0",
                "deposits",
            ]
            .to_vec(),
        ));
        assert_eq!(0, contract_address.len());

        Ok(())
    }

    fn to_key(key: Vec<&str>) -> Vec<String> {
        key.into_iter().map(|k| k.to_string()).collect()
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
