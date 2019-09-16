use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use bytes::{Buf, BufMut, IntoBuf};
use rocksdb::{ColumnFamilyDescriptor, Options, SliceTransform, MergeOperands};
use serde::{Deserialize, Serialize};

use networking::p2p::encoding::prelude::*;
use tezos_encoding::hash::{BlockHash, HashRef, HashType};

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{Codec, Schema, SchemaError, DatabaseWithSchema};
use std::sync::Arc;
use std::convert::TryFrom;

pub type OperationsStorageDatabase = dyn DatabaseWithSchema<OperationsStorage> + Sync + Send;


pub struct OperationsStorage {
    db: Arc<OperationsStorageDatabase>
}

impl OperationsStorage {
    pub fn new(db: Arc<OperationsStorageDatabase>) -> Self {
        OperationsStorage { db }
    }

    pub fn insert(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let key = OperationKey {
            block_hash: HashRef::new(message.operations_for_block.hash.clone()),
            validation_pass: message.operations_for_block.validation_pass as u8
        };
        self.db.put(&key, &message)
            .map_err(|e| e.into())
    }
}

impl Schema for OperationsStorage {
    const COLUMN_FAMILY_NAME: &'static str = "operations_storage";
    type Key = OperationKey;
    type Value = OperationsForBlocksMessage;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(HashType::BlockHash.size()));
        cf_opts.set_memtable_prefix_bloom_ratio(0.2);
        cf_opts.set_merge_operator("operations_storage_merge_operator", merge_meta_value, None);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

/// Layout of the `OperationKey` is:
///
/// * bytes layout: `[block_hash(32)][validation_pass(1)]`
#[derive(Debug, PartialEq)]
pub struct OperationKey {
    block_hash: HashRef,
    validation_pass: u8,
}

impl Codec for OperationKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        Ok(OperationKey {
            block_hash: HashRef::new(bytes[0..HashType::BlockHash.size()].to_vec()),
            validation_pass: bytes[HashType::BlockHash.size()]
        })
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut value = Vec::with_capacity(HashType::BlockHash.size() + 1);
        value.extend(&*self.block_hash.hash);
        value[HashType::BlockHash.size()] = self.validation_pass;
        Ok(value)
    }
}

impl Codec for OperationsForBlocksMessage {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        bincode::deserialize(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        bincode::serialize(self)
            .map_err(|_| SchemaError::DecodeError)
    }
}


fn merge_meta_value(new_key: &[u8], existing_val: Option<&[u8]>, operands: &mut MergeOperands) -> Option<Vec<u8>> {

    let mut result: Vec<u8> = Vec::with_capacity(operands.size_hint().0);
    existing_val.map(|v| {
        for e in v {
            result.push(*e)
        }
    });
    for op in operands {
        for e in op {
            result.push(*e)
        }
    }
    Some(result)
}

/// Convenience type for operation meta storage database
pub type OperationsMetaStorageDatabase = dyn DatabaseWithSchema<OperationsMetaStorage> + Sync + Send;

pub struct OperationsMetaStorage {
    db: Arc<OperationsMetaStorageDatabase>
}

impl OperationsMetaStorage {
    pub fn new(db: Arc<OperationsMetaStorageDatabase>) -> Self {
        OperationsMetaStorage { db }
    }

    pub fn initialize(&mut self, block_header: &BlockHeaderWithHash) -> Result<(), StorageError> {
        self.db.put(&block_header.hash.clone(),
            &Meta {
                validation_passes: block_header.header.validation_pass,
                is_validation_pass_present: vec![false as u8; block_header.header.validation_pass as usize],
                is_complete: false
            }
        ).map_err(|e| e.into())
    }

    pub fn insert(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let block_hash =  HashRef::new(message.operations_for_block.hash.clone());

        match self.db.get(&block_hash)? {
            Some(mut meta) => {
                let validation_pass = message.operations_for_block.validation_pass as u8;

                // update validation passes and check if we have all operations
                meta.is_validation_pass_present[validation_pass as usize] = true as u8;
                meta.is_complete = meta.is_validation_pass_present.iter().all(|v| *v == (true as u8));
                self.db.merge(&block_hash, &meta)
                    .map_err(|e| e.into())
            }
            None => Err(StorageError::MissingKey),
        }
    }

    pub fn get_missing_validation_passes(&mut self, block_hash: &HashRef) -> Result<HashSet<i8>, StorageError> {
        match self.db.get(block_hash)? {
            Some(meta) => {
                let result  = if meta.is_complete {
                    HashSet::new()
                } else {
                    meta.is_validation_pass_present.iter().enumerate()
                        .filter(|(_, is_present)| **is_present == (false as u8))
                        .map(|(idx, _)| idx as i8)
                        .collect()
                };
                Ok(result)
            }
            None => Err(StorageError::MissingKey),
        }
    }

    pub fn is_complete(&self, block_hash: &HashRef) -> Result<bool, StorageError> {
        match self.db.get(block_hash)? {
            Some(Meta { is_complete, .. }) => {
                Ok(is_complete)
            }
            None => Ok(false),
        }
    }


    pub fn contains(&self, block_hash: &HashRef) -> Result<bool, StorageError> {
        self.db.get(block_hash)
            .map(|v| v.is_some())
            .map_err(StorageError::from)
    }
}

impl Schema for OperationsMetaStorage {
    const COLUMN_FAMILY_NAME: &'static str = "operations_meta_storage";
    type Key = HashRef;
    type Value = Meta;
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub struct Meta {
    is_validation_pass_present: Vec<u8>,
    validation_passes: u8,
    is_complete: bool
}

impl Codec for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        bincode::deserialize(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        bincode::serialize(self)
            .map_err(|_| SchemaError::DecodeError)
    }
}

#[cfg(test)]
mod tests {
    use failure::Error;
    use tezos_encoding::hash::HashEncoding;

    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn operations_key_encoded_equals_decoded() -> Result<(), Error> {
        let expected = OperationKey::Message {
            block_hash: HashRef::new(HashEncoding::new(HashType::BlockHash).string_to_bytes("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?),
            validation_pass: 4,
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationKey::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn operations_meta_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Meta {
            is_validation_pass_present: vec![false as u8; 5],
            is_complete: false,
            validation_passes: 5
        };
        let encoded_bytes = expected.encode()?;
        let decoded = OperationValue::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }


    #[test]
    fn mergetest() {
        use rocksdb::{Options, DB};

        let path = "_opstorage_mergetest";
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_merge_operator("test operator", test_provided_merge, None);
        {
            let db = DB::open(&opts, path).unwrap();
            let p = db.put(b"k1", b"a");
            assert!(p.is_ok());
            let _ = db.merge(b"k1", b"b");
            let _ = db.merge(b"k1", b"c");
            let _ = db.merge(b"k1", b"d");
            let _ = db.merge(b"k1", b"efg");
            let m = db.merge(b"k1", b"h");
            assert!(m.is_ok());
            match db.get(b"k1") {
                Ok(Some(value)) => match value.to_utf8() {
                    Some(v) => println!("retrieved utf8 value: {}", v),
                    None => println!("did not read valid utf-8 out of the db"),
                },
                Err(_) => println!("error reading value"),
                _ => panic!("value not present"),
            }

            assert!(m.is_ok());
            let r = db.get(b"k1");
            assert!(r.unwrap().unwrap().to_utf8().unwrap() == "abcdefgh");
            assert!(db.delete(b"k1").is_ok());
            assert!(db.get(b"k1").unwrap().is_none());
        }
        assert!(DB::destroy(&opts, path).is_ok());
    }
}