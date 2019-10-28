// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, MergeOperands, Options};

use tezos_encoding::hash::BlockHash;

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{Codec, DatabaseWithSchema, Schema, SchemaError};
use crate::persistent::database::{IteratorMode, IteratorWithSchema};

pub type BlockMetaStorageDatabase = dyn DatabaseWithSchema<BlockMetaStorage> + Sync + Send;

/// Structure for representing in-memory db for - just for demo purposes.
#[derive(Clone)]
pub struct BlockMetaStorage {
    db: Arc<BlockMetaStorageDatabase>
}

impl BlockMetaStorage {

    pub fn new(db: Arc<BlockMetaStorageDatabase>) -> Self {
        BlockMetaStorage { db }
    }

    pub fn put_block_header(&mut self, block_header: &BlockHeaderWithHash) -> Result<(), StorageError> {
        // create/update record for block
        match self.get(&block_header.hash)?.as_mut() {
            Some(meta) => {
                meta.predecessor = Some(block_header.header.predecessor().clone());
                self.put(&block_header.hash, &meta)?;
            },
            None => {
                let meta = Meta {
                    is_applied: false,
                    predecessor: Some(block_header.header.predecessor().clone()),
                    successor: None,
                    level: block_header.header.level()
                };
                self.put(&block_header.hash, &meta)?;
            }
        }

        // create/update record for block predecessor
        match self.get(&block_header.header.predecessor())?.as_mut() {
            Some(meta) => {
                meta.successor = Some(block_header.hash.clone());
                self.put(block_header.header.predecessor(), &meta)?;
            },
            None => {
                let meta = Meta {
                    is_applied: false,
                    predecessor: None,
                    successor: Some(block_header.hash.clone()),
                    level: block_header.header.level() - 1
                };
                self.put(block_header.header.predecessor(), &meta)?;
            }
        }

        Ok(())
    }

    #[inline]
    pub fn put(&mut self, block_hash: &BlockHash, meta: &Meta) -> Result<(), StorageError> {
        self.db.merge(block_hash, meta)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get(&self, block_hash: &BlockHash) -> Result<Option<Meta>, StorageError> {
        self.db.get(block_hash)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn iter(&self, mode: IteratorMode<Self>) -> Result<IteratorWithSchema<Self>, StorageError> {
        self.db.iterator(mode)
            .map_err(StorageError::from)
    }
}

const BLOCK_HASH_LEN: usize = 32;

const MASK_IS_APPLIED: u8    = 0b0000_0001;
const MASK_HAS_SUCCESSOR: u8   = 0b0000_0010;
const MASK_HAS_PREDECESSOR: u8 = 0b0000_0100;

const IDX_MASK: usize = 0;
const IDX_PREDECESSOR: usize = IDX_MASK + 1;
const IDX_SUCCESSOR: usize = IDX_PREDECESSOR + BLOCK_HASH_LEN;
const IDX_LEVEL: usize = IDX_SUCCESSOR + BLOCK_HASH_LEN;

const BLANK_BLOCK_HASH: [u8; BLOCK_HASH_LEN] = [0; BLOCK_HASH_LEN];
const META_LEN: usize = std::mem::size_of::<u8>() + BLOCK_HASH_LEN + BLOCK_HASH_LEN + std::mem::size_of::<i32>();

macro_rules! is_applied {
    ($mask:expr) => {{ ($mask & MASK_IS_APPLIED) != 0 }}
}
macro_rules! has_predecessor {
    ($mask:expr) => {{ ($mask & MASK_HAS_PREDECESSOR) != 0 }}
}
macro_rules! has_successor {
    ($mask:expr) => {{ ($mask & MASK_HAS_SUCCESSOR) != 0 }}
}

/// Meta information for the block
#[derive(Clone, PartialEq, Debug)]
pub struct Meta {
    pub predecessor: Option<BlockHash>,
    pub successor: Option<BlockHash>,
    pub is_applied: bool,
    pub level: i32
}

impl Meta {
    pub fn genesis_meta(genesis_hash: &BlockHash) -> Self {
        Meta {
            is_applied: true, // we consider genesis as already applied
            predecessor: Some(genesis_hash.clone()), // this is what we want
            successor: None, // we do not know (yet) successor of the genesis
            level: 0
        }
    }
}

/// Codec for `Meta`
///
/// * bytes layout: `[mask(1)][predecessor(32)][successor(32)][level(4)]`
impl Codec for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if META_LEN == bytes.len() {
            let mask = bytes[IDX_MASK];
            let is_processed = is_applied!(mask);
            let predecessor = if has_predecessor!(mask) {
                let block_hash = bytes[IDX_PREDECESSOR..IDX_SUCCESSOR].to_vec();
                assert_eq!(BLOCK_HASH_LEN, block_hash.len(), "Predecessor expected length is {} but found {}", BLOCK_HASH_LEN, block_hash.len());
                Some(block_hash)
            } else {
                None
            };
            let successor = if has_successor!(mask) {
                let block_hash = bytes[IDX_SUCCESSOR..IDX_LEVEL].to_vec();
                assert_eq!(BLOCK_HASH_LEN, block_hash.len(), "Successor expected length is {} but found {}", BLOCK_HASH_LEN, block_hash.len());
                Some(block_hash)
            } else {
                None
            };
            // level
            let mut level_bytes: [u8; 4] = Default::default();
            level_bytes.copy_from_slice(&bytes[IDX_LEVEL..IDX_LEVEL + 4]);
            let level = i32::from_le_bytes(level_bytes);
            Ok(Meta { predecessor, successor, is_applied: is_processed, level })
        } else {
            Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        let mut mask = 0u8;
        if self.is_applied {
            mask |= MASK_IS_APPLIED;
        }
        if self.predecessor.is_some() {
            mask |= MASK_HAS_PREDECESSOR;
        }
        if self.successor.is_some() {
            mask |= MASK_HAS_SUCCESSOR;
        }

        let mut value = Vec::with_capacity(META_LEN);
        value.push(mask);
        match &self.predecessor {
            Some(predecessor) => value.extend(predecessor),
            None => value.extend(&BLANK_BLOCK_HASH)
        }
        match &self.successor {
            Some(successor) => value.extend(successor),
            None => value.extend(&BLANK_BLOCK_HASH)
        }
        value.extend(&self.level.to_le_bytes());
        assert_eq!(META_LEN, value.len(), "Invalid size. predecessor={:?}, successor={:?}, level={:?}, data={:?}", &self.predecessor, &self.successor, self.level, &value);

        Ok(value)
    }
}

impl Schema for BlockMetaStorage {
    const COLUMN_FAMILY_NAME: &'static str = "block_meta_storage";
    type Key = BlockHash;
    type Value = Meta;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_merge_operator("block_meta_storage_merge_operator", merge_meta_value, None);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

fn merge_meta_value(_new_key: &[u8], existing_val: Option<&[u8]>, operands: &mut MergeOperands) -> Option<Vec<u8>> {
    let mut result = existing_val.map(|v| v.to_vec());

    for op in operands {
        match result {
            Some(ref mut val) => {
                assert_eq!(META_LEN, val.len(), "Value length is incorrect. Was expecting {} but instead found {}", META_LEN, val.len());

                let mask_val = val[IDX_MASK];
                let mask_op = op[IDX_MASK];

                // merge `mask(1)`
                val[IDX_MASK] = mask_val | mask_op;

                // if op has predecessor and val has not, copy it from op to val
                if has_predecessor!(mask_op) && !has_predecessor!(mask_val) {
                    val.splice(IDX_PREDECESSOR..IDX_SUCCESSOR, op[IDX_PREDECESSOR..IDX_SUCCESSOR].iter().cloned());
                }
                // if op has successor and val has not, copy it from op to val
                if has_successor!(mask_op) && !has_successor!(mask_val) {
                    val.splice(IDX_SUCCESSOR..IDX_LEVEL, op[IDX_SUCCESSOR..IDX_LEVEL].iter().cloned());
                }
                assert_eq!(META_LEN, val.len(), "Invalid length after merge operator was applied. Was expecting {} but found {}.", META_LEN, val.len());
            },
            None => result = Some(op.to_vec())
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use failure::Error;

    use super::*;
    use tezos_encoding::hash::{HashEncoding, HashType};

    #[test]
    fn block_meta_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Meta {
            is_applied: false,
            predecessor: Some(vec![98; 32]),
            successor: Some(vec![21; 32]),
            level: 34
        };
        let encoded_bytes = expected.encode()?;
        let decoded = Meta::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn genesis_block_initialized_success() -> Result<(), Error> {
        use rocksdb::DB;

        let path = "__blockmeta_genesistest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![BlockMetaStorage::cf_descriptor()]).unwrap();
            let encoding = HashEncoding::new(HashType::BlockHash);

            let k = encoding.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?;
            let v = Meta::genesis_meta(&k);
            let mut storage = BlockMetaStorage::new(Arc::new(db));
            storage.put(&k, &v)?;
            match storage.get(&k)? {
                Some(value) => {
                    let expected = Meta {
                        is_applied: true,
                        predecessor: Some(k.clone()),
                        successor: None,
                        level: 0
                    };
                    assert_eq!(expected, value);
                },
                _ => panic!("value not present"),
            }
        }
        Ok(assert!(DB::destroy(&opts, path).is_ok()))
    }

    #[test]
    fn block_meta_storage_test() -> Result<(), Error> {
        use rocksdb::DB;

        let path = "__blockmeta_storagetest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![BlockMetaStorage::cf_descriptor()]).unwrap();
            let k = vec![44; 32];
            let mut v = Meta {
                is_applied: false,
                predecessor: None,
                successor: None,
                level: 1_245_762
            };
            let mut storage = BlockMetaStorage::new(Arc::new(db));
            storage.put(&k, &v)?;
            let p = storage.get(&k)?;
            assert!(p.is_some());
            v.is_applied = true;
            v.successor = Some(vec![21; 32]);
            storage.put(&k, &v)?;
            v.is_applied = false;
            v.predecessor = Some(vec![98; 32]);
            v.successor = None;
            storage.put(&k, &v)?;
            v.predecessor = None;
            storage.put(&k, &v)?;
            match storage.get(&k)? {
                Some(value) => {
                    let expected = Meta {
                        is_applied: true,
                        predecessor: Some(vec![98; 32]),
                        successor: Some(vec![21; 32]),
                        level: 1_245_762
                    };
                    assert_eq!(expected, value);
                },
                _ => panic!("value not present"),
            }
        }
        Ok(assert!(DB::destroy(&opts, path).is_ok()))
    }

    #[test]
    fn merge_meta_value_test() {
        use rocksdb::{Options, DB};

        let path = "__blockmeta_mergetest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![BlockMetaStorage::cf_descriptor()]).unwrap();
            let k = vec![44; 32];
            let mut v = Meta {
                is_applied: false,
                predecessor: None,
                successor: None,
                level: 2
            };
            let p = BlockMetaStorageDatabase::merge(&db, &k, &v);
            assert!(p.is_ok(), "p: {:?}", p.unwrap_err());
            v.is_applied = true;
            v.successor = Some(vec![21; 32]);
            let _ = BlockMetaStorageDatabase::merge(&db, &k, &v);
            v.is_applied = false;
            v.predecessor = Some(vec![98; 32]);
            v.successor = None;
            let _ = BlockMetaStorageDatabase::merge(&db, &k, &v);
            v.predecessor = None;
            let m = BlockMetaStorageDatabase::merge(&db, &k, &v);
            assert!(m.is_ok());
            match BlockMetaStorageDatabase::get(&db, &k) {
                Ok(Some(value)) => {
                    let expected = Meta {
                        is_applied: true,
                        predecessor: Some(vec![98; 32]),
                        successor: Some(vec![21; 32]),
                        level: 2
                    };
                    assert_eq!(expected, value);
                },
                Err(_) => println!("error reading value"),
                _ => panic!("value not present"),
            }
        }
        assert!(DB::destroy(&opts, path).is_ok());
    }
}