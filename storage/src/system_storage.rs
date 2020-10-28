// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{ColumnFamilyDescriptor, Cache};
use serde::{Deserialize, Serialize};

use crypto::hash::ChainId;

use crate::persistent::{BincodeEncoded, default_table_options, KeyValueSchema, KeyValueStoreWithSchema};
use crate::StorageError;

pub type SystemStorageKv = dyn KeyValueStoreWithSchema<SystemStorage> + Sync + Send;
pub type DbVersion = i64;

/// Represents storage of the system settings.
///
/// This storage differs from the other in regard that it is not exposing key-value pair
/// but instead it provides get_ and set_ methods for each system setting.
#[derive(Clone)]
pub struct SystemStorage {
    kv: Arc<SystemStorageKv>
}

impl SystemStorage {
    const CHAIN_ID: &'static str = "chain_id";
    const DB_VERSION: &'static str = "db_version";
    const CHAIN_NAME: &'static str = "chain_name";

    pub fn new(kv: Arc<SystemStorageKv>) -> Self {
        SystemStorage { kv }
    }

    #[inline]
    pub fn get_chain_id(&self) -> Result<Option<ChainId>, StorageError> {
        self.kv.get(&Self::CHAIN_ID.to_string())
            .map(|result| match result {
                Some(SystemValue::Hash(value)) => Some(value),
                _ => None
            })
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn set_chain_id(&mut self, chain_id: &ChainId) -> Result<(), StorageError> {
        self.kv.put(&Self::CHAIN_ID.to_string(), &SystemValue::Hash(chain_id.clone()))
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get_db_version(&self) -> Result<Option<DbVersion>, StorageError> {
        self.kv.get(&Self::DB_VERSION.to_string())
            .map(|result| match result {
                Some(SystemValue::Integer(value)) => Some(value),
                _ => None
            })
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn set_db_version(&mut self, db_version: DbVersion) -> Result<(), StorageError> {
        self.kv.put(&Self::DB_VERSION.to_string(), &SystemValue::Integer(db_version))
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get_chain_name(&self) -> Result<Option<String>, StorageError> {
        self.kv.get(&Self::CHAIN_NAME.to_string())
            .map(|result| match result {
                Some(SystemValue::String(value)) => Some(value),
                _ => None
            })
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn set_chain_name(&mut self, chain_name: &String) -> Result<(), StorageError> {
        self.kv.put(&Self::CHAIN_NAME.to_string(), &SystemValue::String(chain_name.clone()))
            .map_err(StorageError::from)
    }
}

impl KeyValueSchema for SystemStorage {
    type Key = String;
    type Value = SystemValue;

    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "system_storage"
    }
}

#[derive(Serialize, Deserialize)]
pub enum SystemValue {
    String(String),
    Integer(i64),
    Hash(Vec<u8>),
}

impl BincodeEncoded for SystemValue {}
