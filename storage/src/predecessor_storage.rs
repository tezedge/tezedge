// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::block_meta_storage::Meta;
use crate::database::{DBSubtreeKeyValueSchema, KVDBIteratorWithSchema, KVDBStoreWithSchema};
use crate::persistent::database::IteratorMode;
use crate::persistent::{BincodeEncoded, KeyValueSchema};
use crate::{PersistentStorage, StorageError};

pub type PredecessorsIndexStorageKV = dyn KVDBStoreWithSchema<PredecessorStorage> + Sync + Send;

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
            kv: persistent_storage.main_db(),
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
    pub fn iter(
        &self,
        mode: IteratorMode<Self>,
    ) -> Result<KVDBIteratorWithSchema<Self>, StorageError> {
        self.kv.iterator(mode).map_err(StorageError::from)
    }
}

impl BincodeEncoded for PredecessorKey {}

impl KeyValueSchema for PredecessorStorage {
    type Key = PredecessorKey;
    type Value = BlockHash;
}

impl DBSubtreeKeyValueSchema for PredecessorStorage {
    fn sub_tree_name() -> &'static str {
        "predecessor_storage"
    }
}
