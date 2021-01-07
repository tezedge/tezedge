// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::persistent::database::{IteratorMode, IteratorWithSchema};
use crate::persistent::{
    default_table_options, BincodeEncoded, KeyValueSchema, KeyValueStoreWithSchema,
    PersistentStorage,
};
use crate::StorageError;
use crate::{block_meta_storage::Meta, persistent::StorageType};

pub type PredecessorsIndexStorageKV = dyn KeyValueStoreWithSchema<PredecessorStorage> + Sync + Send;

#[derive(Serialize, Deserialize)]
pub struct PredecessorKey {
    block_hash: BlockHash,
    exponent_slot: u32,
}

impl PredecessorKey {
    pub fn new(block_hash: BlockHash, exponent_slot: u32) -> Self {
        Self {
            block_hash,
            exponent_slot,
        }
    }
}

#[derive(Clone)]
pub struct PredecessorStorage {
    kv: Arc<PredecessorsIndexStorageKV>,
}

impl PredecessorStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.kv(StorageType::Database),
        }
    }

    pub fn store_predecessors(
        &self,
        block_hash: &BlockHash,
        block_meta: &Meta,
        stored_predecessors_size: u32,
    ) -> Result<(), StorageError> {
        if let Some(direct_predecessor) = block_meta.predecessor() {
            // genesis
            if direct_predecessor == block_hash {
                return Ok(());
            } else {
                // put the direct predecessor to slot 0
                self.put(
                    &PredecessorKey::new(block_hash.clone(), 0),
                    direct_predecessor,
                )?;

                // fill other slots
                let mut predecessor = direct_predecessor.clone();
                for predecessor_exponent_slot in 1..stored_predecessors_size {
                    let predecessor_key =
                        PredecessorKey::new(predecessor, predecessor_exponent_slot - 1);
                    if let Some(p) = self.get(&predecessor_key)? {
                        let key =
                            PredecessorKey::new(block_hash.clone(), predecessor_exponent_slot);
                        self.put(&key, &p)?;
                        predecessor = p;
                    } else {
                        return Ok(());
                    }
                }
            }
        }

        Ok(())
    }

    #[inline]
    pub fn put(
        &self,
        key: &PredecessorKey,
        predeccessor_hash: &BlockHash,
    ) -> Result<(), StorageError> {
        self.kv
            .put(key, predeccessor_hash)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get(&self, key: &PredecessorKey) -> Result<Option<BlockHash>, StorageError> {
        self.kv.get(key).map_err(StorageError::from)
    }

    #[inline]
    pub fn iter(&self, mode: IteratorMode<Self>) -> Result<IteratorWithSchema<Self>, StorageError> {
        self.kv.iterator(mode).map_err(StorageError::from)
    }
}

impl BincodeEncoded for PredecessorKey {}

impl KeyValueSchema for PredecessorStorage {
    type Key = PredecessorKey;
    type Value = BlockHash;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "predecessor_storage"
    }
}
