// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::block_meta_storage::Meta;
use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{BincodeEncoded, KeyValueSchema};
use crate::{PersistentStorage, StorageError};

pub type PredecessorsIndexStorageKV =
    dyn TezedgeDatabaseWithIterator<PredecessorStorage> + Sync + Send;

pub const STORED_PREDECESSORS_SIZE: u32 = 12;

#[derive(Serialize, Deserialize, Debug, Clone)]
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
}

impl BincodeEncoded for PredecessorKey {}

impl KeyValueSchema for PredecessorStorage {
    type Key = PredecessorKey;
    type Value = BlockHash;
}

impl RocksDbKeyValueSchema for PredecessorStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "predecessor_storage"
    }
}
impl KVStoreKeyValueSchema for PredecessorStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}

pub trait PredecessorSearch {
    fn get_predecessor_storage(&self) -> PredecessorStorage;

    /// NOTE: implemented in a way to mirro the ocaml code, should be refactored to me more rusty
    ///
    /// Returns n-th predecessor for block_hash
    ///
    /// /// requested_distance - cannot be negative, because we cannot go to top throught successors, because in case of reorg, we dont know which way to choose
    fn find_block_at_distance(
        &self,
        block_hash: BlockHash,
        requested_distance: u32,
    ) -> Result<Option<BlockHash>, StorageError> {
        if requested_distance == 0 {
            return Ok(Some(block_hash));
        }

        let mut distance = requested_distance;
        let mut block_hash = block_hash;
        const BASE: u32 = 2;
        loop {
            if distance == 1 {
                // distance is 1, return the direct prdecessor
                let key = PredecessorKey::new(block_hash, 0);
                return self.get_predecessor_storage().get(&key);
            } else {
                let (mut power, mut rest) = closest_power_two_and_rest(distance);

                if power >= STORED_PREDECESSORS_SIZE {
                    power = STORED_PREDECESSORS_SIZE - 1;
                    rest = distance - BASE.pow(power);
                }

                let key = PredecessorKey::new(block_hash.clone(), power);
                if let Some(pred) = self.get_predecessor_storage().get(&key)? {
                    if rest == 0 {
                        return Ok(Some(pred));
                    } else {
                        block_hash = pred;
                        distance = rest;
                    }
                } else {
                    return Ok(None); // reached genesis
                }
            }
        }
    }
}

/// Function to find the closest power of 2 value to the distance. Returns the closest power
/// and the rest (distance = 2^closest_power + rest)
fn closest_power_two_and_rest(distance: u32) -> (u32, u32) {
    let base: u32 = 2;

    let mut closest_power: u32 = 0;
    let mut rest: u32 = 0;
    let mut distance: u32 = distance;

    while distance > 1 {
        rest += base.pow(closest_power) * (distance % 2);
        distance /= 2;
        closest_power += 1;
    }
    (closest_power, rest)
}
