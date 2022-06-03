// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
// use std::io::SeekFrom;
use std::sync::Arc;

use getset::Getters;
use num::BigInt;
use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::KeyValueSchema;
use crate::{PersistentStorage, StorageError};

pub type RewardStorageKV = dyn TezedgeDatabaseWithIterator<RewardStorage> + Sync + Send;

/// Cycle number
type RewardKey = i32;

#[derive(Clone)]
pub struct RewardStorage {
    kv: Arc<RewardStorageKV>,
}
pub type Delegate = String;
pub type RewardData = BTreeMap<Delegate, CycleRewardsInt>;

// TODO: fuzzing?
/// A struct holding the reward values as mutez strings to be able to serialize BigInts as Strings
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CycleRewards {
    fees: String,
    baking_rewards: String,
    baking_bonuses: String,
    endorsement_rewards: String,
    totoal_rewards: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Getters)]
pub struct CycleRewardsInt {
    #[get = "pub"]
    fees: BigInt,

    #[get = "pub"]
    baking_rewards: BigInt,

    #[get = "pub"]
    baking_bonuses: BigInt,

    #[get = "pub"]
    endorsement_rewards: BigInt,

    pub totoal_rewards: BigInt,
}

impl From<CycleRewardsInt> for CycleRewards {
    fn from(num_ver: CycleRewardsInt) -> Self {
        Self {
            fees: num_ver.fees.to_string(),
            baking_rewards: num_ver.baking_rewards.to_string(),
            endorsement_rewards: num_ver.endorsement_rewards.to_string(),
            baking_bonuses: num_ver.baking_bonuses.to_string(),
            totoal_rewards: num_ver.totoal_rewards.to_string(),
        }
    }
}

impl RewardStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    pub fn store_rewards_data(&self, cycle: i32, rewards: RewardData) -> Result<(), StorageError> {
        self.put(&cycle, rewards)?;

        Ok(())
    }

    #[inline]
    pub fn put(&self, key: &RewardKey, data: RewardData) -> Result<(), StorageError> {
        self.kv.put(key, &data).map_err(StorageError::from)
    }

    pub fn get(&self, key: &RewardKey) -> Result<Option<RewardData>, StorageError> {
        self.kv.get(key).map_err(StorageError::from)
    }
}

impl KeyValueSchema for RewardStorage {
    type Key = RewardKey;
    type Value = RewardData;
}

impl RocksDbKeyValueSchema for RewardStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "reward_storage"
    }
}

impl KVStoreKeyValueSchema for RewardStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
