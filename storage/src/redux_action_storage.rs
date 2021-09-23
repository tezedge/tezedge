// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{BincodeEncoded, Decoder, Encoder, KeyValueSchema};
use crate::{PersistentStorage, StorageError};

pub type ReduxActionIndexStorageKV =
    dyn TezedgeDatabaseWithIterator<ReduxActionStorage> + Sync + Send;

/// Storage for redux::Action.
///
/// Indexed by it's id: ActionId.
#[derive(Clone)]
pub struct ReduxActionStorage {
    kv: Arc<ReduxActionIndexStorageKV>,
}

impl ReduxActionStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    #[inline]
    pub fn put<T>(&self, action_id: &u64, action: &T) -> Result<(), StorageError>
    where
        T: Encoder,
    {
        self.kv
            .put(action_id, &action.encode()?)
            .map_err(StorageError::from)
    }

    #[inline]
    pub fn get<T>(&self, action_id: &u64) -> Result<Option<T>, StorageError>
    where
        T: Decoder,
    {
        let encoded = self.kv.get(action_id).map_err(StorageError::from)?;
        Ok(if let Some(encoded) = encoded {
            Some(T::decode(&encoded)?)
        } else {
            None
        })
    }
}

impl KeyValueSchema for ReduxActionStorage {
    type Key = u64;
    type Value = Vec<u8>;
}

impl RocksDbKeyValueSchema for ReduxActionStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "redux_action_storage"
    }
}

impl KVStoreKeyValueSchema for ReduxActionStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
