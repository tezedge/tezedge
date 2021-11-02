// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::borrow::Cow;
use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{Decoder, Encoder, KeyValueSchema};
use crate::{Direction, IteratorMode, PersistentStorage, StorageError};

pub type ShellAutomatonStateIndexStorageKV =
    dyn TezedgeDatabaseWithIterator<ShellAutomatonStateStorage> + Sync + Send;

/// Storage for shell_automaton::State.
///
/// Indexed by ActionId that modified it [shell_automaton::State::last_action.id].
#[derive(Clone)]
pub struct ShellAutomatonStateStorage {
    kv: Arc<ShellAutomatonStateIndexStorageKV>,
}

impl ShellAutomatonStateStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    #[inline]
    pub fn put<T>(&self, action_id: &u64, state_snapshot: &T) -> Result<(), StorageError>
    where
        T: Encoder,
    {
        self.kv
            .put(action_id, &state_snapshot.encode()?)
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

    /// Get closest state snapshot, where `state.last_action.id` <= `action_id`.
    #[inline]
    pub fn get_closest_before<T>(&self, action_id: &u64) -> Result<Option<T>, StorageError>
    where
        T: Decoder,
    {
        self.kv
            .find(IteratorMode::From(
                Cow::Borrowed(action_id),
                Direction::Reverse,
            ))?
            .take(1)
            .next()
            .map(|res| Ok(T::decode(&res?.1)?))
            .transpose()
    }
}

impl KeyValueSchema for ShellAutomatonStateStorage {
    type Key = u64;
    type Value = Vec<u8>;
}

impl RocksDbKeyValueSchema for ShellAutomatonStateStorage {
    fn descriptor(cache: &Cache) -> ColumnFamilyDescriptor {
        let cf_opts = default_table_options(cache);
        ColumnFamilyDescriptor::new(Self::name(), cf_opts)
    }

    #[inline]
    fn name() -> &'static str {
        "shell_automaton_state_storage"
    }
}

impl KVStoreKeyValueSchema for ShellAutomatonStateStorage {
    fn column_name() -> &'static str {
        Self::name()
    }
}
