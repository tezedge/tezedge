// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use getset::{CopyGetters, Getters, Setters};
use rocksdb::{ColumnFamilyDescriptor, MergeOperands, Cache};
use slog::{Logger, warn};

use crypto::hash::{BlockHash, ChainId, HashType};
use tezos_messages::p2p::encoding::block_header::Level;

use crate::{BlockHeaderWithHash, StorageError};
use crate::num_from_slice;
use crate::persistent::{Decoder, default_table_options, Encoder, KeyValueSchema, KeyValueStoreWithSchema, PersistentStorage, SchemaError};
use crate::persistent::database::{IteratorMode, IteratorWithSchema};

pub type BlockMetaStorageKV = dyn KeyValueStoreWithSchema<BlockMetaStorage> + Sync + Send;

pub trait BlockMetaStorageReader: Sync + Send {
    fn get(&self, block_hash: &BlockHash) -> Result<Option<Meta>, StorageError>;
}

#[derive(Clone)]
pub struct BlockMetaStorage {
    kv: Arc<BlockMetaStorageKV>
}

impl BlockMetaStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        BlockMetaStorage { kv: persistent_storage.kv() }
    }


    /// Creates/updates metadata record in storage from given block header
    /// Returns metadata + true, if it is a new block
    pub fn put_block_header(&self, block_header: &BlockHeaderWithHash, chain_id: &ChainId, log: &Logger) -> Result<(Meta, bool), StorageError> {
        // create/update record for block
        let block_metadata = match self.get(&block_header.hash)? {
            Some(mut meta) => {
                let block_predecessor = block_header.header.predecessor().clone();

                // log if predecessor should be changed - cannot happen
                match &meta.predecessor {
                    None => (),
                    Some(stored_predecessor) => {
                        if *stored_predecessor != block_predecessor {
                            warn!(
                                log, "Detected rewriting predecessor - not allowed (change is just ignored)";
                                "block_hash" => HashType::BlockHash.bytes_to_string(&block_header.hash),
                                "stored_predecessor" => HashType::BlockHash.bytes_to_string(&stored_predecessor),
                                "new_predecessor" => HashType::BlockHash.bytes_to_string(&block_predecessor)
                            );
                        }
                    }
                };

                // predecessor cannot be rewriten (just from None to Some) - see see merge_meta_value
                if meta.predecessor.is_none() {
                    meta.predecessor = Some(block_predecessor);
                    self.put(&block_header.hash, &meta)?;
                }

                (meta, false)
            }
            None => {
                let meta = Meta {
                    is_applied: false,
                    predecessor: Some(block_header.header.predecessor().clone()),
                    successors: vec![],
                    level: block_header.header.level(),
                    chain_id: chain_id.clone(),
                };
                self.put(&block_header.hash, &meta)?;
                (meta, true)
            }
        };

        // create/update record for block predecessor
        match self.get(&block_header.header.predecessor())?.as_mut() {
            Some(meta) => {
                let block_hash = &block_header.hash;

                // log if successor was changed on the predecessor - can happen, means reorg
                let need_change = match meta.successors.is_empty() {
                    true => true,
                    false => {
                        // here we have some previous successors
                        // if does not contains block_hash, means that we detected reorg or new branch
                        if !meta.successors.contains(&block_hash) {
                            warn!(
                                log, "Extending successors - means detected reorg or new branch";
                                "block_hash_predecessor" => HashType::BlockHash.bytes_to_string(&block_header.header.predecessor()),
                                "stored_successors" => {
                                    meta.successors
                                        .iter()
                                        .map(|bh| HashType::BlockHash.bytes_to_string(bh))
                                        .collect::<Vec<String>>()
                                        .join(", ")
                                },
                                "new_successor" => HashType::BlockHash.bytes_to_string(&block_hash)
                            );
                            true
                        } else {
                            false
                        }
                    }
                };

                if need_change {
                    meta.successors.push(block_hash.clone());
                    self.put(block_header.header.predecessor(), &meta)?;
                }
            }
            None => {
                let meta = Meta {
                    is_applied: false,
                    predecessor: None,
                    successors: vec![block_header.hash.clone()],
                    level: block_header.header.level() - 1,
                    chain_id: chain_id.clone(),
                };
                self.put(block_header.header.predecessor(), &meta)?;
            }
        }

        Ok(block_metadata)
    }

    #[inline]
    pub fn put(&self, block_hash: &BlockHash, meta: &Meta) -> Result<(), StorageError> {
        self.kv.merge(block_hash, meta)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get(&self, block_hash: &BlockHash) -> Result<Option<Meta>, StorageError> {
        self.kv.get(block_hash)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn iter(&self, mode: IteratorMode<Self>) -> Result<IteratorWithSchema<Self>, StorageError> {
        self.kv.iterator(mode)
            .map_err(StorageError::from)
    }
}

impl BlockMetaStorageReader for BlockMetaStorage {
    fn get(&self, block_hash: &BlockHash) -> Result<Option<Meta>, StorageError> {
        self.kv.get(block_hash)
            .map_err(StorageError::from)
    }
}

const LEN_BLOCK_HASH: usize = HashType::BlockHash.size();
const LEN_CHAIN_ID: usize = HashType::ChainId.size();

const MASK_IS_APPLIED: u8 = 0b0000_0001;
const MASK_HAS_SUCCESSOR: u8 = 0b0000_0010;
const MASK_HAS_PREDECESSOR: u8 = 0b0000_0100;

const IDX_MASK: usize = 0;
const IDX_PREDECESSOR: usize = IDX_MASK + 1;
const IDX_LEVEL: usize = IDX_PREDECESSOR + LEN_BLOCK_HASH;
const IDX_CHAIN_ID: usize = IDX_LEVEL + std::mem::size_of::<i32>();
const IDX_SUCCESSOR_COUNT: usize = IDX_CHAIN_ID + LEN_CHAIN_ID;
const IDX_SUCCESSOR: usize = IDX_SUCCESSOR_COUNT + std::mem::size_of::<usize>();

const BLANK_BLOCK_HASH: [u8; LEN_BLOCK_HASH] = [0; LEN_BLOCK_HASH];
const LEN_FIXED_META: usize = std::mem::size_of::<u8>() + LEN_BLOCK_HASH + std::mem::size_of::<i32>() + LEN_CHAIN_ID + std::mem::size_of::<usize>();

const fn total_len(predecessors_count: usize) -> usize {
    LEN_FIXED_META + (predecessors_count * LEN_BLOCK_HASH)
}

macro_rules! is_applied {
    ($mask:expr) => {{ ($mask & MASK_IS_APPLIED) != 0 }}
}
macro_rules! has_predecessor {
    ($mask:expr) => {{ ($mask & MASK_HAS_PREDECESSOR) != 0 }}
}
macro_rules! has_successor {
    ($mask:expr) => {{ ($mask & MASK_HAS_SUCCESSOR) != 0 }}
}
macro_rules! successors_count {
    ($bytes:expr) => {{ num_from_slice!($bytes, IDX_SUCCESSOR_COUNT, usize) }}
}

/// Meta information for the block
#[derive(Clone, Getters, CopyGetters, Setters, PartialEq, Debug)]
pub struct Meta {
    #[get = "pub"]
    predecessor: Option<BlockHash>,
    #[get = "pub"]
    successors: Vec<BlockHash>,
    #[get_copy = "pub"]
    #[set = "pub"]
    is_applied: bool,
    #[get_copy = "pub"]
    level: Level,
    #[get = "pub"]
    chain_id: ChainId,
}

impl Meta {
    /// Create Metadata for specific genesis block
    pub fn genesis_meta(genesis_hash: &BlockHash, genesis_chain_id: &ChainId, is_applied: bool) -> Self {
        Meta {
            is_applied,
            predecessor: Some(genesis_hash.clone()), // this is what we want
            successors: vec![], // we do not know (yet) successor of the genesis
            level: 0,
            chain_id: genesis_chain_id.clone(),
        }
    }
}

/// Codec for `Meta`
///
/// * bytes layout: `[mask(1)][predecessor(32)][level(4)][chain_id(4)][successors_count(8)][successors(successors_count*32)]`
impl Decoder for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if LEN_FIXED_META <= bytes.len() {
            // mask
            let mask = bytes[IDX_MASK];
            let is_processed = is_applied!(mask);
            // predecessor
            let predecessor = if has_predecessor!(mask) {
                let block_hash = bytes[IDX_PREDECESSOR..IDX_LEVEL].to_vec();
                assert_eq!(LEN_BLOCK_HASH, block_hash.len(), "Predecessor expected length is {} but found {}", LEN_BLOCK_HASH, block_hash.len());
                Some(block_hash)
            } else {
                None
            };
            // level
            let level = num_from_slice!(bytes, IDX_LEVEL, i32);
            // chain_id
            let chain_id = bytes[IDX_CHAIN_ID..IDX_SUCCESSOR_COUNT].to_vec();

            // successors
            let successors = {
                let count = successors_count!(bytes);
                let mut successors = Vec::with_capacity(count);
                if has_successor!(mask) {
                    for i in 0..count {
                        let next_successor_index = IDX_SUCCESSOR + (i * LEN_BLOCK_HASH);
                        let block_hash = bytes[next_successor_index..(next_successor_index + LEN_BLOCK_HASH)].to_vec();
                        assert_eq!(LEN_BLOCK_HASH, block_hash.len(), "Successor expected length is {} but found {}", LEN_BLOCK_HASH, block_hash.len());
                        successors.push(block_hash);
                    }
                }
                successors
            };

            assert_eq!(LEN_CHAIN_ID, chain_id.len(), "Chain ID expected length is {} but found {}", LEN_CHAIN_ID, chain_id.len());
            Ok(Meta { predecessor, successors, is_applied: is_processed, level, chain_id })
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
        if !self.successors.is_empty() {
            mask |= MASK_HAS_SUCCESSOR;
        }
        let successors_count = self.successors.len();
        let total_len = total_len(successors_count);

        let mut value = Vec::with_capacity(total_len);
        value.push(mask);
        match &self.predecessor {
            Some(predecessor) => value.extend(predecessor),
            None => value.extend(&BLANK_BLOCK_HASH)
        }
        value.extend(&self.level.to_be_bytes());
        value.extend(&self.chain_id);
        value.extend(&successors_count.to_be_bytes());
        if successors_count > 0 {
            self.successors
                .iter()
                .for_each(|successor| {
                    value.extend(successor);
                });
        }
        assert_eq!(total_len, value.len(), "Invalid size. mask={:?}, predecessor={:?}, successors={:?}, level={:?}, data={:?}", mask, &self.predecessor, &self.successors, self.level, &value);

        Ok(value)
    }
}

impl KeyValueSchema for BlockMetaStorage {
    type Key = BlockHash;
    type Value = Meta;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options(cache);
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
                assert!(LEN_FIXED_META <= val.len(), "Value length is incorrect. Was expecting at least {} but instead found {}", LEN_FIXED_META, val.len());

                let mask_val = val[IDX_MASK];
                let mask_op = op[IDX_MASK];

                // merge `mask(1)`
                val[IDX_MASK] = mask_val | mask_op;

                // if op has predecessor and val has not, copy it from op to val
                if has_predecessor!(mask_op) && !has_predecessor!(mask_val) {
                    val.splice(IDX_PREDECESSOR..IDX_LEVEL, op[IDX_PREDECESSOR..IDX_LEVEL].iter().cloned());
                }

                // replace op (successors count + successors) to val
                let val_successors_count = successors_count!(val);
                let op_successors_count = successors_count!(op);
                if (has_successor!(mask_op) && !has_successor!(mask_val)) || (val_successors_count != op_successors_count) {
                    val.truncate(LEN_FIXED_META);
                    val.splice(IDX_SUCCESSOR_COUNT.., op[IDX_SUCCESSOR_COUNT..].iter().cloned());
                }

                let total_len = total_len(op_successors_count);
                assert_eq!(total_len, val.len(), "Invalid length after merge operator was applied. Was expecting {} but found {}.", total_len, val.len());
            }
            None => result = Some(op.to_vec())
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use failure::Error;

    use crypto::hash::HashType;

    use crate::persistent::{DbConfiguration, open_kv};
    use crate::tests_common::TmpStorage;

    use super::*;

    #[test]
    fn block_meta_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Meta {
            is_applied: false,
            predecessor: Some(vec![98; 32]),
            successors: vec![vec![21; 32]],
            level: 34,
            chain_id: vec![44; 4],
        };
        let encoded_bytes = expected.encode()?;
        let decoded = Meta::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn genesis_block_initialized_success() -> Result<(), Error> {
        let tmp_storage = TmpStorage::create("__blockmeta_genesistest")?;

        let k = HashType::BlockHash.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?;
        let chain_id = HashType::ChainId.string_to_bytes("NetXgtSLGNJvNye")?;
        let v = Meta::genesis_meta(&k, &chain_id, true);
        let storage = BlockMetaStorage::new(tmp_storage.storage());
        storage.put(&k, &v)?;
        match storage.get(&k)? {
            Some(value) => {
                let expected = Meta {
                    is_applied: true,
                    predecessor: Some(k.clone()),
                    successors: vec![],
                    level: 0,
                    chain_id: chain_id.clone(),
                };
                assert_eq!(expected, value);
            }
            _ => panic!("value not present"),
        }

        Ok(())
    }

    #[test]
    fn block_meta_storage_test() -> Result<(), Error> {
        let tmp_storage = TmpStorage::create("__blockmeta_storagetest")?;

        let k = vec![44; 32];
        let mut v = Meta {
            is_applied: false,
            predecessor: None,
            successors: vec![],
            level: 1_245_762,
            chain_id: vec![44; 4],
        };
        let storage = BlockMetaStorage::new(tmp_storage.storage());
        storage.put(&k, &v)?;
        assert!(storage.get(&k)?.is_some());

        // change applied to true and predecessor + add successor
        v.is_applied = true;
        v.predecessor = Some(vec![98; 32]);
        v.successors = vec![vec![21; 32]];
        storage.put(&k, &v)?;

        // try change is_applied (cannot be overwritten - see merge_meta_value)
        v.is_applied = false;
        storage.put(&k, &v)?;

        // try change predecessor (cannot be overwritten - see merge_meta_value)
        v.predecessor = Some(vec![198; 32]);
        storage.put(&k, &v)?;

        // add successor
        v.successors = vec![vec![21; 32], vec![121; 32]];
        storage.put(&k, &v)?;

        // check stored meta
        match storage.get(&k)? {
            Some(value) => {
                let expected = Meta {
                    is_applied: true,
                    predecessor: Some(vec![98; 32]),
                    successors: vec![vec![21; 32], vec![121; 32]],
                    level: 1_245_762,
                    chain_id: vec![44; 4],
                };
                assert_eq!(expected, value);
            }
            _ => panic!("value not present"),
        }

        // try remove predecesor (cannot be overwritten - see merge_meta_value)
        v.predecessor = None;
        storage.put(&k, &v)?;

        // modify successors
        v.successors = vec![vec![121; 32]];
        storage.put(&k, &v)?;

        // check stored meta
        match storage.get(&k)? {
            Some(value) => {
                let expected = Meta {
                    is_applied: true,
                    predecessor: Some(vec![98; 32]),
                    successors: vec![vec![121; 32]],
                    level: 1_245_762,
                    chain_id: vec![44; 4],
                };
                assert_eq!(expected, value);
            }
            _ => panic!("value not present"),
        }

        Ok(())
    }

    #[test]
    fn merge_meta_value_test() -> Result<(), Error> {
        use rocksdb::{Options, DB, Cache};

        let path = "__blockmeta_mergetest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        {
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
            let db = open_kv(path, vec![BlockMetaStorage::descriptor(&cache)], &DbConfiguration::default()).unwrap();
            let k = vec![44; 32];
            let mut v = Meta {
                is_applied: false,
                predecessor: None,
                successors: vec![],
                level: 2,
                chain_id: vec![44; 4],
            };
            let p = BlockMetaStorageKV::merge(&db, &k, &v);
            assert!(p.is_ok(), "p: {:?}", p.unwrap_err());
            v.is_applied = true;
            v.successors = vec![vec![21; 32]];
            let _ = BlockMetaStorageKV::merge(&db, &k, &v);

            v.is_applied = false;
            v.predecessor = Some(vec![98; 32]);
            v.successors = vec![];
            let _ = BlockMetaStorageKV::merge(&db, &k, &v);

            v.predecessor = None;
            let m = BlockMetaStorageKV::merge(&db, &k, &v);
            assert!(m.is_ok());

            match BlockMetaStorageKV::get(&db, &k) {
                Ok(Some(value)) => {
                    let expected = Meta {
                        is_applied: true,
                        predecessor: Some(vec![98; 32]),
                        successors: vec![],
                        level: 2,
                        chain_id: vec![44; 4],
                    };
                    assert_eq!(expected, value);
                }
                Err(_) => println!("error reading value"),
                _ => panic!("value not present"),
            }
        }
        Ok(assert!(DB::destroy(&Options::default(), path).is_ok()))
    }
}