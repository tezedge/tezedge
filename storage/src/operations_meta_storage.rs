// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, MergeOperands, Options};

use tezos_encoding::hash::{BlockHash, ChainId, HashType};
use tezos_messages::p2p::encoding::prelude::*;

use crate::{BlockHeaderWithHash, StorageError};
use crate::persistent::{Codec, DatabaseWithSchema, Schema, SchemaError};
use crate::persistent::database::{IteratorMode, IteratorWithSchema};

/// Convenience type for operation meta storage database
pub type OperationsMetaStorageDatabase = dyn DatabaseWithSchema<OperationsMetaStorage> + Sync + Send;

/// Operation metadata storage
#[derive(Clone)]
pub struct OperationsMetaStorage {
    db: Arc<OperationsMetaStorageDatabase>
}

impl OperationsMetaStorage {
    pub fn new(db: Arc<OperationsMetaStorageDatabase>) -> Self {
        OperationsMetaStorage { db }
    }

    #[inline]
    pub fn put_block_header(&mut self, block_header: &BlockHeaderWithHash, chain_id: &ChainId) -> Result<(), StorageError> {
        self.put(&block_header.hash.clone(),
                 &Meta {
                     validation_passes: block_header.header.validation_pass(),
                     is_validation_pass_present: vec![false as u8; block_header.header.validation_pass() as usize],
                     is_complete: block_header.header.validation_pass() == 0,
                     level: block_header.header.level(),
                     chain_id: chain_id.clone(),
                 },
        )
    }

    pub fn put_operations(&mut self, message: &OperationsForBlocksMessage) -> Result<(), StorageError> {
        let block_hash = message.operations_for_block().hash().clone();

        match self.get(&block_hash)? {
            Some(mut meta) => {
                let validation_pass = message.operations_for_block().validation_pass() as u8;

                // update validation passes and check if we have all operations
                meta.is_validation_pass_present[validation_pass as usize] = true as u8;
                meta.is_complete = meta.is_validation_pass_present.iter().all(|v| *v == (true as u8));
                self.put(&block_hash, &meta)
            }
            None => Err(StorageError::MissingKey),
        }
    }

    #[inline]
    pub fn is_complete(&self, block_hash: &BlockHash) -> Result<bool, StorageError> {
        match self.get(block_hash)? {
            Some(Meta { is_complete, .. }) => {
                Ok(is_complete)
            }
            None => Ok(false),
        }
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
    pub fn contains(&self, block_hash: &BlockHash) -> Result<bool, StorageError> {
        self.db.contains(block_hash)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn iter(&self, mode: IteratorMode<Self>) -> Result<IteratorWithSchema<Self>, StorageError> {
        self.db.iterator(mode)
            .map_err(StorageError::from)
    }
}

impl Schema for OperationsMetaStorage {
    const COLUMN_FAMILY_NAME: &'static str = "operations_meta_storage";
    type Key = BlockHash;
    type Value = Meta;

    fn cf_descriptor() -> ColumnFamilyDescriptor {
        let mut cf_opts = Options::default();
        cf_opts.set_merge_operator("operations_meta_storage_merge_operator", merge_meta_value, None);
        ColumnFamilyDescriptor::new(Self::COLUMN_FAMILY_NAME, cf_opts)
    }
}

fn merge_meta_value(_new_key: &[u8], existing_val: Option<&[u8]>, operands: &mut MergeOperands) -> Option<Vec<u8>> {
    let mut result = existing_val.map(|v| v.to_vec());

    for op in operands {
        match result {
            Some(ref mut val) => {
                assert_eq!(val.len(), op.len(), "Value length is fixed. expected={}, found={}", val.len(), op.len());
                assert_ne!(0, val.len(), "Value cannot have zero size");
                assert_eq!(val[0], op[0], "Value of validation passes cannot change");

                let validation_passes = val[0] as usize;
                // merge `is_validation_pass_present`
                for i in 1..=validation_passes {
                    val[i] |= op[i]
                }
                // merge `is_complete`
                let is_complete_idx = validation_passes + 1;
                val[is_complete_idx] |= op[is_complete_idx];
            }
            None => result = Some(op.to_vec())
        }
    }

    result
}

/// Block operations metadata
#[derive(PartialEq, Debug)]
pub struct Meta {
    validation_passes: u8,
    is_validation_pass_present: Vec<u8>,
    is_complete: bool,
    level: i32,
    pub chain_id: ChainId,
}

impl Meta {
    pub fn is_complete(&self) -> bool {
        self.is_complete
    }

    pub fn genesis_meta(chain_id: &ChainId) -> Self {
        Meta {
            is_complete: true,
            validation_passes: 0,
            is_validation_pass_present: vec![],
            level: 0,
            chain_id: chain_id.clone(),
        }
    }

    pub fn get_missing_validation_passes(&self) -> HashSet<i8> {
        if self.is_complete {
            HashSet::new()
        } else {
            self.is_validation_pass_present.iter().enumerate()
                .filter(|(_, is_present)| **is_present == (false as u8))
                .map(|(idx, _)| idx as i8)
                .collect()
        }
    }

    #[inline]
    pub fn level(&self) -> i32 {
        self.level
    }
}

/// Codec for `Meta`
///
/// * bytes layout: `[validation_passes(1)][is_validation_pass_present(validation_passes * 1)][is_complete(1)][level(4)][chain_id(4)]`
impl Codec for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if !bytes.is_empty() {
            let validation_passes = bytes[0];
            assert_eq!(expected_data_length(validation_passes), bytes.len(),
                       "Was expecting the length of a block with validation_passes={} to be {} but instead it was {}", validation_passes, expected_data_length(validation_passes), bytes.len());
            let is_complete_pos = validation_passes as usize + 1;
            let is_validation_pass_present = bytes[1..is_complete_pos].to_vec();
            let is_complete = bytes[is_complete_pos] != 0;
            // level
            let level_pos = is_complete_pos + 1;
            let mut level_bytes: [u8; 4] = Default::default();
            level_bytes.copy_from_slice(&bytes[level_pos..level_pos + 4]);
            let level = i32::from_le_bytes(level_bytes);
            assert!(level >= 0, "Level must be positive number, but instead it is: {}", level);
            // chain_id
            let chain_id_pos = level_pos + level_bytes.len();
            let chain_id = bytes[chain_id_pos..chain_id_pos + HashType::ChainId.size()].to_vec();
            Ok(Meta { validation_passes, is_validation_pass_present, is_complete, level, chain_id })
        } else {
            Err(SchemaError::DecodeError)
        }
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        if (self.validation_passes as usize) == self.is_validation_pass_present.len() {
            let mut value = vec![];
            value.push(self.validation_passes);
            value.extend(&self.is_validation_pass_present);
            value.push(self.is_complete as u8);
            value.extend(&self.level.to_le_bytes());
            value.extend(&self.chain_id);
            assert_eq!(expected_data_length(self.validation_passes), value.len(), "Was expecting value to have length {} but instead found {}", expected_data_length(self.validation_passes), value.len());
            Ok(value)
        } else {
            Err(SchemaError::EncodeError)
        }
    }
}

#[inline]
fn expected_data_length(validation_passes: u8) -> usize {
    std::mem::size_of::<u8>()           // validation_passes
        + std::mem::size_of::<u8>()     // is_complete
        + std::mem::size_of::<i32>()    // level
        + (validation_passes as usize) * std::mem::size_of::<u8>()  // is_validation_pass_present
        + HashType::ChainId.size()     // chain_id
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use failure::Error;

    use tezos_encoding::hash::{HashEncoding, HashType};

    use super::*;

    #[test]
    fn operations_meta_encoded_equals_decoded() -> Result<(), Error> {
        let expected = Meta {
            is_validation_pass_present: vec![false as u8; 5],
            is_complete: false,
            validation_passes: 5,
            level: 93_422,
            chain_id: vec![44; 4],
        };
        let encoded_bytes = expected.encode()?;
        let decoded = Meta::decode(&encoded_bytes)?;
        Ok(assert_eq!(expected, decoded))
    }

    #[test]
    fn genesis_ops_initialized_success() -> Result<(), Error> {
        use rocksdb::DB;

        let path = "__opmeta_genesistest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![OperationsMetaStorage::cf_descriptor()]).unwrap();
            let encoding = HashEncoding::new(HashType::BlockHash);

            let k = encoding.string_to_bytes("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?;
            let v = Meta::genesis_meta(&vec![44; 4]);
            let mut storage = OperationsMetaStorage::new(Arc::new(db));
            storage.put(&k, &v)?;
            match storage.get(&k)? {
                Some(value) => {
                    let expected = Meta {
                        validation_passes: 0,
                        is_validation_pass_present: vec![],
                        is_complete: true,
                        level: 0,
                        chain_id: vec![44; 4],
                    };
                    assert_eq!(expected, value);
                }
                _ => panic!("value not present"),
            }
        }
        Ok(assert!(DB::destroy(&opts, path).is_ok()))
    }

    #[test]
    fn operations_meta_storage_test() -> Result<(), Error> {
        use rocksdb::DB;

        let path = "__opmeta_storagetest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let t = true as u8;
            let f = false as u8;

            let db = DB::open_cf_descriptors(&opts, path, vec![OperationsMetaStorage::cf_descriptor()]).unwrap();
            let k = vec![3, 1, 3, 3, 7];
            let mut v = Meta {
                is_complete: false,
                is_validation_pass_present: vec![f; 5],
                validation_passes: 5,
                level: 785,
                chain_id: vec![44; 4],
            };
            let mut storage = OperationsMetaStorage::new(Arc::new(db));
            storage.put(&k, &v)?;
            v.is_validation_pass_present[2] = t;
            storage.put(&k, &v)?;
            v.is_validation_pass_present[2] = f;
            v.is_validation_pass_present[3] = t;
            storage.put(&k, &v)?;
            v.is_validation_pass_present[3] = f;
            v.is_complete = true;
            storage.put(&k, &v)?;
            storage.put(&k, &v)?;
            match storage.get(&k)? {
                Some(value) => {
                    assert_eq!(vec![f, f, t, t, f], value.is_validation_pass_present);
                    assert!(value.is_complete);
                    assert_eq!(785, value.level);
                    assert_eq!(vec![44; 4], value.chain_id);
                }
                _ => panic!("value not present"),
            }
        }
        Ok(assert!(DB::destroy(&opts, path).is_ok()))
    }

    #[test]
    fn merge_meta_value_test() {
        use rocksdb::{Options, DB};

        let path = "__opmeta_storage_mergetest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        opts.set_merge_operator("test operator", merge_meta_value, None);
        {
            let t = true as u8;
            let f = false as u8;

            let db = DB::open_cf_descriptors(&opts, path, vec![OperationsMetaStorage::cf_descriptor()]).unwrap();
            let k = vec![3, 1, 3, 3, 7];
            let mut v = Meta {
                is_complete: false,
                is_validation_pass_present: vec![f; 5],
                validation_passes: 5,
                level: 31_337,
                chain_id: vec![44; 4],
            };
            let p = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            assert!(p.is_ok(), "p: {:?}", p.unwrap_err());
            v.is_validation_pass_present[2] = t;
            let _ = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            v.is_validation_pass_present[2] = f;
            v.is_validation_pass_present[3] = t;
            let _ = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            v.is_validation_pass_present[3] = f;
            v.is_complete = true;
            let _ = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            let m = OperationsMetaStorageDatabase::merge(&db, &k, &v);
            assert!(m.is_ok());
            match OperationsMetaStorageDatabase::get(&db, &k) {
                Ok(Some(value)) => {
                    assert_eq!(vec![f, f, t, t, f], value.is_validation_pass_present);
                    assert!(value.is_complete);
                    assert_eq!(31_337, value.level);
                    assert_eq!(vec![44; 4], value.chain_id);
                }
                Err(_) => println!("error reading value"),
                _ => panic!("value not present"),
            }
        }
        assert!(DB::destroy(&opts, path).is_ok());
    }

    #[test]
    fn operations_meta_storage_test_contains() -> Result<(), Error> {
        use rocksdb::DB;

        let path = "__opmeta_storage_test_contains";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.create_missing_column_families(true);
        {
            let db = DB::open_cf_descriptors(&opts, path, vec![OperationsMetaStorage::cf_descriptor()]).unwrap();
            let k = vec![3, 1, 3, 3, 7];
            let v = Meta {
                is_complete: false,
                is_validation_pass_present: vec![false as u8; 5],
                validation_passes: 5,
                level: 785,
                chain_id: vec![44; 4],
            };
            let k_missing_1 = vec![0, 1, 2];
            let k_added_later = vec![6, 7, 8, 9];

            let mut storage = OperationsMetaStorage::new(Arc::new(db));
            assert!(!storage.contains(&k)?);
            assert!(!storage.contains(&k_missing_1)?);
            assert!(!storage.contains(&k_added_later)?);
            storage.put(&k, &v)?;
            assert!(storage.contains(&k)?);
            assert!(!storage.contains(&k_missing_1)?);
            assert!(!storage.contains(&k_added_later)?);
            storage.put(&k_added_later, &v)?;
            assert!(storage.contains(&k)?);
            assert!(!storage.contains(&k_missing_1)?);
            assert!(storage.contains(&k_added_later)?);
        }
        Ok(assert!(DB::destroy(&opts, path).is_ok()))
    }
}