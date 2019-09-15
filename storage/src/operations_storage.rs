use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform};

use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{HashRef, HashType, BlockHash};

use crate::BlockHeaderWithHash;
use crate::persistent::{Schema, Codec, SchemaError};

pub struct Operations {
    pub(crate) block_hash: HashRef,
    values: Vec<Option<OperationsForBlocksMessage>>
}

impl PartialEq for Operations {
    fn eq(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
    }
}

impl Eq for Operations {}

impl Hash for Operations {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.block_hash.hash(state);
    }
}

impl Operations {

    pub fn insert(&mut self, value: OperationsForBlocksMessage) {
        let op_idx = value.operations_for_block.validation_pass as usize;
        self.values[op_idx] = Some(value);
    }

    pub fn is_complete(&self) -> bool {
        self.values.iter().all(Option::is_some)
    }

    pub fn get_missing_validation_passes(&self) -> HashSet<i8> {
        self.values.iter().enumerate()
            .filter(|(_, value)| value.is_none())
            .map(|(idx, _)| idx as i8)
            .collect()
    }
}

/// Structure for representing in-memory db for - just for demo purposes.
pub struct OperationsStorage(HashMap<HashRef, Operations>);

impl OperationsStorage {

    pub fn new() -> Self {
        OperationsStorage(HashMap::new())
    }

    pub fn initialize_operations(&mut self, block_header: &BlockHeaderWithHash) {
        let operations = Operations {
            block_hash: block_header.hash.clone(),
            values: vec![None; block_header.header.validation_pass as usize],
        };
        self.0.insert(block_header.hash.clone(), operations);
    }

    pub fn get_operations_mut(&mut self, block_hash: &HashRef) -> Option<&mut Operations> {
        self.0.get_mut(block_hash)
    }

    pub fn get_operations(&mut self, block_hash: &HashRef) -> Option<&Operations> {
        self.0.get(block_hash)
    }

    pub fn contains(&self, block_hash: &HashRef) -> bool {
        self.0.contains_key(block_hash)
    }
}

enum KeyType {
    Meta,
    Message {
        validation_pass: u8
    }
}

struct OperationKey {
    block_hash: BlockHash,
    key_type: KeyType
}

impl Codec for OperationKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        let block_hash_size = HashType::BlockHash.size();
        let block_hash = bytes[0..block_hash_size].to_vec();
        let key_type = match bytes[block_hash_size] {
            0 => KeyType::Meta,
            1 => KeyType::Message { validation_pass: block_hash[block_hash_size + 1] }
            _ => SchemaError::DecodeError
        };
        Ok(OperationKey { block_hash, key_type })
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut value = self.block_hash.clone();
        match self.key_type {
            KeyType::Meta => value.push(0),
            KeyType::Message { validation_pass } => {
                value.push(1);
                value.push(validation_pass);
            }
        }
        Ok(value)
    }
}

impl Schema for OperationsStorage {
    const COLUMN_FAMILY_NAME: &'static str = "operations_storage";
    type Key = OperationKey;
    type Value = ();

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(HashType::BlockHash.size()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }

}