// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::Arc;

use rocksdb::{Cache, ColumnFamilyDescriptor};
use serde::{Deserialize, Serialize};

use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezedgeDatabaseWithIterator};
use crate::persistent::database::{default_table_options, RocksDbKeyValueSchema};
use crate::persistent::{Decoder, Encoder, KeyValueSchema, SchemaError};
use crate::{PersistentStorage, StorageError};

pub type ShellAutomatonActionMetaIndexStorageKV =
    dyn TezedgeDatabaseWithIterator<ShellAutomatonActionMetaStorage> + Sync + Send;

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum ShellAutomatonActionMetaKey {
    Stats = 1,
    Graph,
}

impl Encoder for ShellAutomatonActionMetaKey {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        Ok(vec![*self as u8])
    }
}

impl Decoder for ShellAutomatonActionMetaKey {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        let key = match bytes.get(0) {
            Some(v) => *v,
            None => return Err(SchemaError::DecodeError),
        };

        use ShellAutomatonActionMetaKey::*;

        Ok(match key {
            x if x == Stats as u8 => Stats,
            x if x == Graph as u8 => Graph,
            _ => return Err(SchemaError::DecodeError),
        })
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ShellAutomatonActionStats {
    /// Total number of times this action kind was executed.
    pub total_calls: u64,
    /// Sum of durations from this action till the next one in nanoseconds.
    pub total_duration: u64,
    /// Max duration.
    pub max_duration: u64,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ShellAutomatonActionStatsForRanges {
    pub under_1_us: ShellAutomatonActionStats,
    pub under_10_us: ShellAutomatonActionStats,
    pub under_50_us: ShellAutomatonActionStats,
    pub under_100_us: ShellAutomatonActionStats,
    pub under_500_us: ShellAutomatonActionStats,
    pub under_1_ms: ShellAutomatonActionStats,
    pub under_5_ms: ShellAutomatonActionStats,
    pub above_5_ms: ShellAutomatonActionStats,
}

impl ShellAutomatonActionStatsForRanges {
    pub fn add(&mut self, duration: u64) {
        let stats = if duration <= 1_000 {
            &mut self.under_1_us
        } else if duration <= 10_000 {
            &mut self.under_10_us
        } else if duration <= 50_000 {
            &mut self.under_50_us
        } else if duration <= 100_000 {
            &mut self.under_100_us
        } else if duration <= 500_000 {
            &mut self.under_500_us
        } else if duration <= 1_000_000 {
            &mut self.under_1_ms
        } else if duration <= 5_000_000 {
            &mut self.under_5_ms
        } else {
            &mut self.above_5_ms
        };
        stats.total_calls += 1;
        stats.total_duration += duration;
        stats.max_duration = std::cmp::max(stats.max_duration, duration);
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ShellAutomatonActionsStats {
    pub stats: HashMap<String, ShellAutomatonActionStatsForRanges>,
}

impl ShellAutomatonActionsStats {
    pub fn new() -> Self {
        Default::default()
    }
}

impl From<HashMap<String, ShellAutomatonActionStatsForRanges>> for ShellAutomatonActionsStats {
    fn from(stats: HashMap<String, ShellAutomatonActionStatsForRanges>) -> Self {
        Self { stats }
    }
}

impl crate::persistent::BincodeEncoded for ShellAutomatonActionsStats {}

/// Storage for shell_automaton::Action.
///
/// Indexed by it's id: ActionId.
#[derive(Clone)]
pub struct ShellAutomatonActionMetaStorage {
    kv: Arc<ShellAutomatonActionMetaIndexStorageKV>,
}

impl ShellAutomatonActionMetaStorage {
    pub fn new(persistent_storage: &PersistentStorage) -> Self {
        Self {
            kv: persistent_storage.main_db(),
        }
    }

    #[inline]
    pub fn get_stats(&self) -> Result<Option<ShellAutomatonActionsStats>, StorageError> {
        Ok(self
            .kv
            .get(&ShellAutomatonActionMetaKey::Stats)?
            .map(|encoded| ShellAutomatonActionsStats::decode(&encoded))
            .transpose()?)
    }

    #[inline]
    pub fn set_stats(&self, meta: &ShellAutomatonActionsStats) -> Result<(), StorageError> {
        Ok(self
            .kv
            .put(&ShellAutomatonActionMetaKey::Stats, &meta.encode()?)?)
    }
    #[inline]
    pub fn get_graph<T>(&self) -> Result<Option<T>, StorageError>
    where
        T: Decoder,
    {
        Ok(self
            .kv
            .get(&ShellAutomatonActionMetaKey::Graph)?
            .map(|encoded| T::decode(&encoded))
            .transpose()?)
    }

    #[inline]
    pub fn set_graph<T>(&self, graph: &T) -> Result<(), StorageError>
    where
        T: Encoder,
    {
        Ok(self
            .kv
            .put(&ShellAutomatonActionMetaKey::Graph, &graph.encode()?)?)
    }
}

impl KeyValueSchema for ShellAutomatonActionMetaStorage {
    type Key = ShellAutomatonActionMetaKey;
    type Value = Vec<u8>;
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
