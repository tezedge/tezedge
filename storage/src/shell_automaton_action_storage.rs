// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{BincodeEncoded, Decoder, Encoder, KeyValueSchema, SchemaError};
use crate::{Direction, IteratorMode, PersistentStorage, StorageError};

pub type ShellAutomatonActionIndexStorageKV =
    dyn TezedgeDatabaseWithIterator<ShellAutomatonActionStorage> + Sync + Send;

/// Storage for shell_automaton::Action.
///
/// Indexed by it's id: ActionId.
#[derive(Clone)]
pub struct ShellAutomatonActionStorage {
    kv: Arc<ShellAutomatonActionIndexStorageKV>,
}

impl ShellAutomatonActionStorage {
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

    #[inline]
    pub fn actions_before<T, F>(
        &self,
        action_id: u64,
        limit: Option<usize>,
        filter: F,
    ) -> Result<impl DoubleEndedIterator<Item = (u64, T)>, StorageError>
    where
        T: Decoder,
        F: 'static + Fn(u64, &T) -> bool,
    {
        let results = self.kv.find(
            IteratorMode::From(&action_id, Direction::Reverse),
            limit,
            Box::new(move |(k, v)| {
                let key = u64::decode(k)?;
                let value = T::decode(v)?;
                Ok(filter(key, &value))
            }),
        )?;

        Ok(results
            .into_iter()
            .map(|(k, v)| Result::<_, SchemaError>::Ok((u64::decode(&k)?, T::decode(&v)?)))
            // in `find` filter method, we do decoding, so decoding cant
            // fail here.
            .filter_map(Result::ok))
    }

    #[inline]
    pub fn actions_after<T, F>(
        &self,
        action_id: u64,
        limit: Option<usize>,
        filter: F,
    ) -> Result<impl DoubleEndedIterator<Item = (u64, T)>, StorageError>
    where
        T: Decoder,
        F: 'static + Fn(u64, &T) -> bool,
    {
        let results = self.kv.find(
            IteratorMode::From(&action_id, Direction::Forward),
            limit,
            Box::new(move |(k, v)| {
                let key = u64::decode(k)?;
                let value = T::decode(v)?;
                Ok(filter(key, &value))
            }),
        )?;

        Ok(results
            .into_iter()
            .map(|(k, v)| Result::<_, SchemaError>::Ok((u64::decode(&k)?, T::decode(&v)?)))
            // in `find` filter method, we do decoding, so decoding cant
            // fail here.
            .filter_map(Result::ok))
    }
}

impl KeyValueSchema for ShellAutomatonActionStorage {
    type Key = u64;
    type Value = Vec<u8>;
}

impl RocksDbKeyValueSchema for ShellAutomatonActionStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "shell_automaton_action_storage"
    }
}

impl KVStoreKeyValueSchema for ShellAutomatonActionStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
