// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use getset::Getters;
use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use tezos_api::ffi::CycleRollsOwnerSnapshot;

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{BincodeEncoded, KeyValueSchema};
use crate::{IteratorMode, PersistentStorage, StorageError};

pub type CycleMetaStorageKV = dyn TezedgeDatabaseWithIterator<CycleMetaStorage> + Sync + Send;

type CycleKey = i32;

#[derive(Clone)]
pub struct CycleMetaStorage {
    kv: Arc<CycleMetaStorageKV>,
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Getters)]
pub struct CycleData {
    #[get = "pub"]
    seed_bytes: Vec<u8>,

    #[get = "pub"]
    rolls_data: Vec<(Vec<u8>, Vec<i32>)>,

    #[get = "pub"]
    last_roll: i32,
}

impl From<CycleRollsOwnerSnapshot> for CycleData {
    fn from(data: CycleRollsOwnerSnapshot) -> CycleData {
        CycleData {
            seed_bytes: data.seed_bytes,
            rolls_data: data.rolls_data,
            last_roll: data.last_roll,
        }
    }
}

impl CycleData {
    pub fn new(seed_bytes: Vec<u8>, rolls_data: Vec<(Vec<u8>, Vec<i32>)>, last_roll: i32) -> Self {
        Self {
            seed_bytes,
            rolls_data,
            last_roll,
        }
    }
}

impl CycleMetaStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    pub fn store_cycle_data(
        &self,
        ffi_cycle_data: CycleRollsOwnerSnapshot,
    ) -> Result<(), StorageError> {
        // add some additional logic here if necessary
        self.put(&ffi_cycle_data.cycle, &ffi_cycle_data.clone().into())?;

        Ok(())
    }

    #[inline]
    pub fn put(&self, key: &CycleKey, data: &CycleData) -> Result<(), StorageError> {
        self.kv.put(key, data).map_err(StorageError::from)
    }

    pub fn get(&self, key: &CycleKey) -> Result<Option<CycleData>, StorageError> {
        self.kv.get(key).map_err(StorageError::from)
    }

    pub fn iterator(&self) -> Result<Vec<(CycleKey, CycleData)>, StorageError> {
        self.kv
            .find(IteratorMode::Start)?
            .map(|result| {
                let result = result?;
                let k = {
                    use crate::persistent::codec::Decoder;
                    <Self as KeyValueSchema>::Key::decode(&result.0)?
                };
                let v = <Self as KeyValueSchema>::Value::decode(&result.1)?;
                Ok((k, v))
            })
            .collect()
    }
}

impl BincodeEncoded for CycleData {}

impl KeyValueSchema for CycleMetaStorage {
    type Key = CycleKey;
    type Value = CycleData;
}

impl RocksDbKeyValueSchema for CycleMetaStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "cycle_data_storage"
    }
}

impl KVStoreKeyValueSchema for CycleMetaStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
