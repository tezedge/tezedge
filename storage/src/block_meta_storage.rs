// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, MergeOperands, Options};

use tezos_encoding::hash::{BlockHash, ChainId, HashType};

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{DatabaseWithSchema, KeyValueSchema, Decoder, Encoder, SchemaError};
use crate::persistent::database::{IteratorMode, IteratorWithSchema};

pub type BlockMetaStorageDatabase = dyn DatabaseWithSchema<BlockMetaStorage> + Sync + Send;

#[derive(Clone)]
pub struct BlockMetaStorage {
    db: Arc<BlockMetaStorageDatabase>
}

impl BlockMetaStorage {
    pub fn new(db: Arc<BlockMetaStorageDatabase>) -> Self {
        BlockMetaStorage { db }
    }

    /// Create new metadata record in storage from given block header
    pub fn put_block_header(&mut self, block_header: &BlockHeaderWithHash, chain_id: &ChainId) -> Result<(), StorageError> {
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
                    level: block_header.header.level(),
                    chain_id: chain_id.clone(),
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
                    level: block_header.header.level() - 1,
                    chain_id: chain_id.clone(),
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

const LEN_BLOCK_HASH: usize = HashType::BlockHash.size();
const LEN_CHAIN_ID: usize = HashType::ChainId.size();

const MASK_IS_APPLIED: u8    = 0b0000_0001;
const MASK_HAS_SUCCESSOR: u8   = 0b0000_0010;
const MASK_HAS_PREDECESSOR: u8 = 0b0000_0100;

const IDX_MASK: usize = 0;
const IDX_PREDECESSOR: usize = IDX_MASK + 1;
const IDX_SUCCESSOR: usize = IDX_PREDECESSOR + LEN_BLOCK_HASH;
const IDX_LEVEL: usize = IDX_SUCCESSOR + LEN_BLOCK_HASH;
const IDX_CHAIN_ID: usize = IDX_LEVEL + std::mem::size_of::<i32>();
const IDX_END: usize = IDX_CHAIN_ID + LEN_CHAIN_ID;

const BLANK_BLOCK_HASH: [u8; LEN_BLOCK_HASH] = [0; LEN_BLOCK_HASH];
const LEN_META: usize = std::mem::size_of::<u8>() + LEN_BLOCK_HASH + LEN_BLOCK_HASH + std::mem::size_of::<i32>() + LEN_CHAIN_ID;

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
    pub level: i32,
    pub chain_id: ChainId,
}

impl Meta {
    /// Create Metadata for specific genesis block
    pub fn genesis_meta(genesis_hash: &BlockHash, genesis_chain_id: &ChainId) -> Self {
        Meta {
            is_applied: true, // we consider genesis as already applied
            predecessor: Some(genesis_hash.clone()), // this is what we want
            successor: None, // we do not know (yet) successor of the genesis
            level: 0,
            chain_id: genesis_chain_id.clone(),
        }
    }
}

/// Codec for `Meta`
///
/// * bytes layout: `[mask(1)][predecessor(32)][successor(32)][level(4)][chain_id(4)]`
impl Decoder for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if LEN_META == bytes.len() {
            // mask
            let mask = bytes[IDX_MASK];
            let is_processed = is_applied!(mask);
            // predecessor
            let predecessor = if has_predecessor!(mask) {
                let block_hash = bytes[IDX_PREDECESSOR..IDX_SUCCESSOR].to_vec();
                assert_eq!(LEN_BLOCK_HASH, block_hash.len(), "Predecessor expected length is {} but found {}", LEN_BLOCK_HASH, block_hash.len());
                Some(block_hash)
            } else {
                None
            };
            // successor
            let successor = if has_successor!(mask) {
                let block_hash = bytes[IDX_SUCCESSOR..IDX_LEVEL].to_vec();
                assert_eq!(LEN_BLOCK_HASH, block_hash.len(), "Successor expected length is {} but found {}", LEN_BLOCK_HASH, block_hash.len());
                Some(block_hash)
            } else {
                None
            };
            // level
            let mut level_bytes: [u8; 4] = Default::default();
            level_bytes.copy_from_slice(&bytes[IDX_LEVEL..IDX_LEVEL + 4]);
            let level = i32::from_le_bytes(level_bytes);
            // chain_id
            let chain_id = bytes[IDX_CHAIN_ID..IDX_END].to_vec();
            assert_eq!(LEN_CHAIN_ID, chain_id.len(), "Chain ID expected length is {} but found {}", LEN_CHAIN_ID, chain_id.len());
            Ok(Meta { predecessor, successor, is_applied: is_processed, level, chain_id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

impl Encoder for Meta {
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

        let mut value = Vec::with_capacity(LEN_META);
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
        value.extend(&self.chain_id);
        assert_eq!(LEN_META, value.len(), "Invalid size. predecessor={:?}, successor={:?}, level={:?}, data={:?}", &self.predecessor, &self.successor, self.level, &value);

        Ok(value)
    }
}

impl KeyValueSchema for BlockMetaStorage {
    type Key = BlockHash;
    type Value = Meta;

    fn descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_merge_operator("block_meta_storage_merge_operator", merge_meta_value, None);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "block_meta_storage"
    }
}

fn merge_meta_value(_new_key: &[u8], existing_val: Option<&[u8]>, operands: &mut MergeOperands) -> Option<Vec<u8>> {
    let mut result = existing_val.map(|v| v.to_vec());

    for op in operands {
        match result {
            Some(ref mut val) => {
                assert_eq!(LEN_META, val.len(), "Value length is incorrect. Was expecting {} but instead found {}", LEN_META, val.len());

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
                assert_eq!(LEN_META, val.len(), "Invalid length after merge operator was applied. Was expecting {} but found {}.", LEN_META, val.len());
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

    use tezos_encoding::hash::{HashEncoding, HashType};

    use super::*;

    #[test]
    fn block_meta_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Meta {
            is_applied: false,
            predecessor: Some(vec![98; 32]),
            successor: Some(vec![21; 32]),
            level: 34,
            chain_id: vec![44; 4],
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
            let db = DB::open_cf_descriptors(&opts, path, vec![BlockMetaStorage::descriptor()]).unwrap();
            let encoding = HashEncoding::new(HashType::BlockHash);

            let k = encoding.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?;
            let chain_id = HashEncoding::new(HashType::ChainId).string_to_bytes("NetXgtSLGNJvNye")?;
            let v = Meta::genesis_meta(&k, &chain_id);
            let mut storage = BlockMetaStorage::new(Arc::new(db));
            storage.put(&k, &v)?;
            match storage.get(&k)? {
                Some(value) => {
                    let expected = Meta {
                        is_applied: true,
                        predecessor: Some(k.clone()),
                        successor: None,
                        level: 0,
                        chain_id: chain_id.clone(),
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
            let db = DB::open_cf_descriptors(&opts, path, vec![BlockMetaStorage::descriptor()]).unwrap();
            let k = vec![44; 32];
            let mut v = Meta {
                is_applied: false,
                predecessor: None,
                successor: None,
                level: 1_245_762,
                chain_id: vec![44; 4],
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
                        level: 1_245_762,
                        chain_id: vec![44; 4],
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
            let db = DB::open_cf_descriptors(&opts, path, vec![BlockMetaStorage::descriptor()]).unwrap();
            let k = vec![44; 32];
            let mut v = Meta {
                is_applied: false,
                predecessor: None,
                successor: None,
                level: 2,
                chain_id: vec![44; 4],
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
                        level: 2,
                        chain_id: vec![44; 4],
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