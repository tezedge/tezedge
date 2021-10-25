// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{BincodeEncoded, Decoder, Encoder, KeyValueSchema};
use crate::{PersistentStorage, StorageError};

pub type ShellAutomatonActionMetaIndexStorageKV =
    dyn TezedgeDatabaseWithIterator<ShellAutomatonActionMetaStorage> + Sync + Send;

#[derive(Clone, Copy)]
pub struct ShellAutomatonActionMetaKey;

impl Encoder for ShellAutomatonActionMetaKey {
    fn encode(&self) -> Result<Vec<u8>, crate::persistent::SchemaError> {
        Ok(vec![0])
    }
}

impl Decoder for ShellAutomatonActionMetaKey {
    fn decode(_: &[u8]) -> Result<Self, crate::persistent::SchemaError> {
        Ok(ShellAutomatonActionMetaKey {})
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShellAutomatonActionMeta {
    /// Total number of times this action kind was executed.
    pub total_calls: u64,
    /// Sum of durations from this action till the next one in nanoseconds.
    pub total_duration: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ShellAutomatonActionMetas {
    pub metas: HashMap<String, ShellAutomatonActionMeta>,
}

impl ShellAutomatonActionMetas {
    pub fn new() -> Self {
        Self {
            metas: HashMap::new(),
        }
    }
}

impl BincodeEncoded for ShellAutomatonActionMetas {}

/// Storage for shell_automaton::Action.
///
/// Indexed by it's id: ActionId.
#[derive(Clone)]
pub struct ShellAutomatonActionMetaStorage {
    kv: Arc<ShellAutomatonActionMetaIndexStorageKV>,
}

impl ShellAutomatonActionMetaStorage {
    const KEY: ShellAutomatonActionMetaKey = ShellAutomatonActionMetaKey {};

    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    #[inline]
    pub fn get(&self) -> Result<Option<ShellAutomatonActionMetas>, StorageError> {
        self.kv.get(&Self::KEY).map_err(StorageError::from)
    }

    #[inline]
    pub fn set(&self, meta: &ShellAutomatonActionMetas) -> Result<(), StorageError> {
        self.kv.put(&Self::KEY, meta).map_err(StorageError::from)
    }
}

impl KeyValueSchema for ShellAutomatonActionMetaStorage {
    type Key = ShellAutomatonActionMetaKey;
    type Value = ShellAutomatonActionMetas;
}

impl RocksDbKeyValueSchema for ShellAutomatonActionMetaStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "shell_automaton_action_meta_storage"
    }
}

impl KVStoreKeyValueSchema for ShellAutomatonActionMetaStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
