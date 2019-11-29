// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_encoding::hash::ChainId;

use crate::persistent::{BincodeEncoded, DatabaseWithSchema, KeyValueSchema};
use crate::StorageError;

pub type SystemStorageDatabase = dyn DatabaseWithSchema<SystemStorage> + Sync + Send;
pub type DbVersion = i64;

/// Represents storage of the system settings.
///
/// This storage differs from the other in regard that it is not exposing key-value pair
/// but instead it provides get_ and set_ methods for each system setting.
#[derive(Clone)]
pub struct SystemStorage {
    db: Arc<SystemStorageDatabase>
}

impl SystemStorage {

    const CHAIN_ID: &'static str = "chain_id";
    const DB_VERSION: &'static str = "db_version";

    pub fn new(db: Arc<SystemStorageDatabase>) -> Self {
        SystemStorage { db }
    }

    #[inline]
    pub fn get_chain_id(&self) -> Result<Option<ChainId>, StorageError> {
        self.db.get(&Self::CHAIN_ID.to_string())
            .map(|result|  match result {
                Some(SystemValue::Hash(value)) => Some(value),
                _ => None
            })
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn set_chain_id(&mut self, chain_id: &ChainId) -> Result<(), StorageError> {
        self.db.put(&Self::CHAIN_ID.to_string(), &SystemValue::Hash(chain_id.clone()))
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get_db_version(&self) -> Result<Option<DbVersion>, StorageError> {
        self.db.get(&Self::DB_VERSION.to_string())
            .map(|result|  match result {
                Some(SystemValue::Integer(value)) => Some(value),
                _ => None
            })
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn set_db_version(&mut self, db_version: DbVersion) -> Result<(), StorageError> {
        self.db.put(&Self::DB_VERSION.to_string(), &SystemValue::Integer(db_version))
            .map_err(StorageError::from)
    }
}


impl KeyValueSchema for SystemStorage {
    type Key = String;
    type Value = SystemValue;

    #[inline]
    fn name() -> &'static str {
        "system_storage"
    }
}

#[derive(Serialize, Deserialize)]
pub enum SystemValue {
    String(String),
    Integer(i64),
    Hash(Vec<u8>)
}

impl BincodeEncoded for SystemValue {}