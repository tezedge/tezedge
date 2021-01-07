// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor, MergeOperands};

use crypto::hash::{BlockHash, ChainId, HashType};
use tezos_messages::p2p::encoding::prelude::*;

use crate::persistent::database::{IteratorMode, IteratorWithSchema};
use crate::persistent::{
    default_table_options, Decoder, Encoder, KeyValueSchema, KeyValueStoreWithSchema,
    PersistentStorage, SchemaError,
};
use crate::{num_from_slice, persistent::StorageType};
use crate::{BlockHeaderWithHash, StorageError};

/// Convenience type for operation meta storage database
pub type OperationsMetaStorageKV = dyn KeyValueStoreWithSchema<OperationsMetaStorage> + Sync + Send;

/// Operation metadata storage
#[derive(Clone)]
pub struct OperationsMetaStorage {
    kv: Arc<OperationsMetaStorageKV>,
}

impl OperationsMetaStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.kv(StorageType::Database),
        }
    }

    /// Initialize stored validation_passes metadata for block and check if is_complete
    #[inline]
    pub fn put_block_header(
        &self,
        block_header: &BlockHeaderWithHash,
        chain_id: &ChainId,
    ) -> Result<bool, StorageError> {
        let meta = Meta {
            validation_passes: block_header.header.validation_pass(),
            is_validation_pass_present: vec![
                false as u8;
                block_header.header.validation_pass() as usize
            ],
            is_complete: block_header.header.validation_pass() == 0,
            level: block_header.header.level(),
            chain_id: chain_id.clone(),
        };
        self.put(&block_header.hash, &meta)
            .and(Ok(meta.is_complete))
    }

    /// Stores operation validation_passes metadata and check if is_complete
    pub fn put_operations(
        &self,
        message: &OperationsForBlocksMessage,
    ) -> Result<bool, StorageError> {
        let block_hash = message.operations_for_block().hash().clone();

        match self.get(&block_hash)? {
            Some(mut meta) => {
                let validation_pass = message.operations_for_block().validation_pass() as u8;

                // update validation passes and check if we have all operations
                meta.is_validation_pass_present[validation_pass as usize] = true as u8;
                meta.is_complete = meta
                    .is_validation_pass_present
                    .iter()
                    .all(|v| *v == (true as u8));
                self.put(&block_hash, &meta).and(Ok(meta.is_complete))
            }
            None => Err(StorageError::MissingKey),
        }
    }

    #[inline]
    pub fn is_complete(&self, block_hash: &BlockHash) -> Result<bool, StorageError> {
        match self.get(block_hash)? {
            Some(Meta { is_complete, .. }) => Ok(is_complete),
            None => Ok(false),
        }
    }

    #[inline]
    pub fn put(&self, block_hash: &BlockHash, meta: &Meta) -> Result<(), StorageError> {
        self.kv.merge(block_hash, meta).map_err(StorageError::from)
    }

    #[inline]
    pub fn get(&self, block_hash: &BlockHash) -> Result<Option<Meta>, StorageError> {
        self.kv.get(block_hash).map_err(StorageError::from)
    }

    #[inline]
    pub fn contains(&self, block_hash: &BlockHash) -> Result<bool, StorageError> {
        self.kv.contains(block_hash).map_err(StorageError::from)
    }

    #[inline]
    pub fn iter(&self, mode: IteratorMode<Self>) -> Result<IteratorWithSchema<Self>, StorageError> {
        self.kv.iterator(mode).map_err(StorageError::from)
    }
}

impl KeyValueSchema for OperationsMetaStorage {
    type Key = BlockHash;
    type Value = Meta;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let mut cf_opts = default_table_options(cache);
        cf_opts.set_merge_operator(
            "operations_meta_storage_merge_operator",
            merge_meta_value,
            None,
        );
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "operations_meta_storage"
    }
}

fn merge_meta_value(
    _new_key: &[u8],
    existing_val: Option<&[u8]>,
    operands: &mut MergeOperands,
) -> Option<Vec<u8>> {
    let mut result = existing_val.map(|v| v.to_vec());

    for op in operands {
        match result {
            Some(ref mut val) => {
                assert_eq!(
                    val.len(),
                    op.len(),
                    "Value length is fixed. expected={}, found={}",
                    val.len(),
                    op.len()
                );
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
            None => result = Some(op.to_vec()),
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
    chain_id: ChainId,
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
            self.is_validation_pass_present
                .iter()
                .enumerate()
                .filter(|(_, is_present)| **is_present == (false as u8))
                .map(|(idx, _)| idx as i8)
                .collect()
        }
    }

    #[inline]
    pub fn level(&self) -> i32 {
        self.level
    }

    #[inline]
    pub fn chain_id(&self) -> &ChainId {
        &self.chain_id
    }
}

/// Codec for `Meta`
///
/// * bytes layout: `[validation_passes(1)][is_validation_pass_present(validation_passes * 1)][is_complete(1)][level(4)][chain_id(4)]`
impl Decoder for Meta {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        if !bytes.is_empty() {
            let validation_passes = bytes[0];
            assert_eq!(expected_data_length(validation_passes), bytes.len(),
                       "Was expecting the length of a block with validation_passes={} to be {} but instead it was {}", validation_passes, expected_data_length(validation_passes), bytes.len());
            let is_complete_pos = validation_passes as usize + 1;
            let is_validation_pass_present = bytes[1..is_complete_pos].to_vec();
            let is_complete = bytes[is_complete_pos] != 0;
            // level
            let level_pos = is_complete_pos + std::mem::size_of::<u8>();
            let level = num_from_slice!(bytes, level_pos, i32);
            assert!(
                level >= 0,
                "Level must be positive number, but instead it is: {}",
                level
            );
            // chain_id
            let chain_id_pos = level_pos + std::mem::size_of::<i32>();
            let chain_id = bytes[chain_id_pos..chain_id_pos + HashType::ChainId.size()].to_vec();
            Ok(Meta {
                validation_passes,
                is_validation_pass_present,
                is_complete,
                level,
                chain_id,
            })
        } else {
            Err(SchemaError::DecodeError)
        }
    }
}

impl Encoder for Meta {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        if (self.validation_passes as usize) == self.is_validation_pass_present.len() {
            let mut value = vec![];
            value.push(self.validation_passes);
            value.extend(&self.is_validation_pass_present);
            value.push(self.is_complete as u8);
            value.extend(&self.level.to_be_bytes());
            value.extend(&self.chain_id);
            assert_eq!(
                expected_data_length(self.validation_passes),
                value.len(),
                "Was expecting value to have length {} but instead found {}",
                expected_data_length(self.validation_passes),
                value.len()
            );
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
        + HashType::ChainId.size() // chain_id
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use failure::Error;

    use crypto::hash::HashType;

    use crate::persistent::{open_kv, DbConfiguration};
    use crate::tests_common::TmpStorage;

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
        assert_eq!(expected, decoded);
        Ok(())
    }

    #[test]
    fn genesis_ops_initialized_success() -> Result<(), Error> {
        let tmp_storage = TmpStorage::create("__opmeta_genesistest")?;

        let k = HashType::BlockHash
            .b58check_to_hash("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")?;
        let v = Meta::genesis_meta(&vec![44; 4]);
        let storage = OperationsMetaStorage::new(tmp_storage.storage());
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

        Ok(())
    }

    #[test]
    fn operations_meta_storage_test() -> Result<(), Error> {
        let tmp_storage = TmpStorage::create("__opmeta_storagetest")?;

        let t = true as u8;
        let f = false as u8;

        let k = vec![3, 1, 3, 3, 7];
        let mut v = Meta {
            is_complete: false,
            is_validation_pass_present: vec![f; 5],
            validation_passes: 5,
            level: 785,
            chain_id: vec![44; 4],
        };
        let storage = OperationsMetaStorage::new(tmp_storage.storage());
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

        Ok(())
    }

    #[test]
    fn merge_meta_value_test() -> Result<(), Error> {
        use rocksdb::{Cache, Options, DB};

        let path = "__opmeta_storage_mergetest";
        if Path::new(path).exists() {
            std::fs::remove_dir_all(path).unwrap();
        }

        {
            let t = true as u8;
            let f = false as u8;
            let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();

            let db = open_kv(
                path,
                vec![OperationsMetaStorage::descriptor(&cache)],
                &DbConfiguration::default(),
            )?;
            let k = vec![3, 1, 3, 3, 7];
            let mut v = Meta {
                is_complete: false,
                is_validation_pass_present: vec![f; 5],
                validation_passes: 5,
                level: 31_337,
                chain_id: vec![44; 4],
            };
            let p = OperationsMetaStorageKV::merge(&db, &k, &v);
            assert!(p.is_ok(), "p: {:?}", p.unwrap_err());
            v.is_validation_pass_present[2] = t;
            let _ = OperationsMetaStorageKV::merge(&db, &k, &v);
            v.is_validation_pass_present[2] = f;
            v.is_validation_pass_present[3] = t;
            let _ = OperationsMetaStorageKV::merge(&db, &k, &v);
            v.is_validation_pass_present[3] = f;
            v.is_complete = true;
            let _ = OperationsMetaStorageKV::merge(&db, &k, &v);
            let m = OperationsMetaStorageKV::merge(&db, &k, &v);
            assert!(m.is_ok());
            match OperationsMetaStorageKV::get(&db, &k) {
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
        assert!(DB::destroy(&Options::default(), path).is_ok());
        Ok(())
    }

    #[test]
    fn operations_meta_storage_test_contains() -> Result<(), Error> {
        let tmp_storage = TmpStorage::create("__opmeta_storage_test_contains")?;

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

        let storage = OperationsMetaStorage::new(tmp_storage.storage());
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

        Ok(())
    }
}
