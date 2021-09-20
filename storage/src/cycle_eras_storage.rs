// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// use std::io::SeekFrom;
use std::sync::Arc;

use getset::Getters;
use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crypto::hash::ProtocolHash;

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{BincodeEncoded, KeyValueSchema};
use crate::{PersistentStorage, StorageError};

pub type CycleErasStorageKV = dyn TezedgeDatabaseWithIterator<CycleErasStorage> + Sync + Send;

// TODO: double check the type
type CycleErasKey = ProtocolHash;

#[derive(Clone)]
pub struct CycleErasStorage {
    kv: Arc<CycleErasStorageKV>,
}

pub type CycleErasData = Vec<CycleEra>;

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq, Getters)]
pub struct CycleEra {
    #[get = "pub"]
    first_level: i32,

    #[get = "pub"]
    first_cycle: i32,

    #[get = "pub"]
    blocks_per_cycle: i32,

    #[get = "pub"]
    blocks_per_commitment: i32,
}

impl CycleErasStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    pub fn store_cycle_eras_data(
        &self,
        protocol_hash: ProtocolHash,
        new_cycle_eras_json: String,
    ) -> Result<(), StorageError> {
        // deserialize the JSON into the CycleErasData struct
        let decoded_cycle_eras: CycleErasData = serde_json::from_str(&new_cycle_eras_json)?;

        self.put(&protocol_hash, decoded_cycle_eras)?;

        Ok(())
    }

    #[inline]
    fn put(&self, key: &CycleErasKey, data: CycleErasData) -> Result<(), StorageError> {
        self.kv.put(key, &data).map_err(StorageError::from)
    }

    pub fn get(&self, key: &CycleErasKey) -> Result<Option<CycleErasData>, StorageError> {
        self.kv.get(key).map_err(StorageError::from)
    }
}

impl BincodeEncoded for CycleErasData {}

impl KeyValueSchema for CycleErasStorage {
    type Key = CycleErasKey;
    type Value = CycleErasData;
}

impl RocksDbKeyValueSchema for CycleErasStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "cycle_eras_storage"
    }
}

impl KVStoreKeyValueSchema for CycleErasStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
