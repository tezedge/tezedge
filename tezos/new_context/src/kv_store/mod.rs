// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This sub module provides different KV alternatives for context persistence

use std::str::FromStr;
use std::{
    convert::{TryFrom, TryInto},
    num::NonZeroUsize,
};

use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

use crate::EntryHash;

pub mod entries;
pub mod in_memory;
pub mod readonly_ipc;
pub mod stats;

pub const INMEM: &str = "inmem";

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct HashId(NonZeroUsize); // NonZeroUsize so that `Option<HashId>` is 8 bytes

pub struct HashIdError;

impl TryInto<usize> for HashId {
    type Error = HashIdError;

    fn try_into(self) -> Result<usize, Self::Error> {
        self.0.get().checked_sub(1).ok_or(HashIdError)
    }
}

impl TryFrom<usize> for HashId {
    type Error = HashIdError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        value
            .checked_add(1)
            .and_then(NonZeroUsize::new)
            .map(HashId)
            .ok_or(HashIdError)
    }
}

const SHIFT: usize = (std::mem::size_of::<usize>() * 8) - 1;
const READONLY: usize = 1 << SHIFT;

impl HashId {
    fn set_readonly_runner(&mut self) -> Result<(), HashIdError> {
        let hash_id = self.0.get();

        self.0 = NonZeroUsize::new(hash_id | READONLY).ok_or(HashIdError)?;

        Ok(())
    }

    fn get_readonly_id(self) -> Result<Option<HashId>, HashIdError> {
        let hash_id = self.0.get();
        if hash_id & READONLY != 0 {
            Ok(Some(HashId(
                NonZeroUsize::new(hash_id & !READONLY).ok_or(HashIdError)?,
            )))
        } else {
            Ok(None)
        }
    }
}

pub struct VacantEntryHash<'a> {
    entry: Option<&'a mut EntryHash>,
    hash_id: HashId,
}

impl<'a> VacantEntryHash<'a> {
    pub(crate) fn write_with<F>(self, fun: F) -> HashId
    where
        F: FnOnce(&mut EntryHash),
    {
        if let Some(entry) = self.entry {
            fun(entry)
        };
        self.hash_id
    }

    pub(crate) fn set_readonly_runner(mut self) -> Result<Self, HashIdError> {
        self.hash_id.set_readonly_runner()?;
        Ok(self)
    }
}

#[derive(PartialEq, Eq, Hash, Debug, Clone, EnumIter)]
pub enum SupportedContextKeyValueStore {
    InMem,
}

impl SupportedContextKeyValueStore {
    pub fn possible_values() -> Vec<&'static str> {
        let mut possible_values = Vec::new();
        for sp in SupportedContextKeyValueStore::iter() {
            possible_values.extend(sp.supported_values());
        }
        possible_values
    }

    fn supported_values(&self) -> Vec<&'static str> {
        match self {
            SupportedContextKeyValueStore::InMem => vec!["inmem"],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ParseKeyValueStoreBackendError(String);

impl FromStr for SupportedContextKeyValueStore {
    type Err = ParseKeyValueStoreBackendError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.to_ascii_lowercase();
        for sp in SupportedContextKeyValueStore::iter() {
            if sp.supported_values().contains(&s.as_str()) {
                return Ok(sp);
            }
        }

        Err(ParseKeyValueStoreBackendError(format!(
            "Invalid variant name: {}",
            s
        )))
    }
}

pub mod test_support {
    use std::collections::HashMap;
    use std::convert::TryFrom;
    use std::path::PathBuf;

    use strum::IntoEnumIterator;

    use crate::persistent::Persistable;
    use crate::working_tree::Entry;
    use crate::ContextKeyValueStore;
    use crate::EntryHash;

    use super::SupportedContextKeyValueStore;

    pub type TestKeyValueStoreError = failure::Error;
    pub type TestContextKvStoreFactoryInstance = Box<dyn TestContextKvStoreFactory>;

    pub fn blob(value: Vec<u8>) -> Entry {
        Entry::Blob(value)
    }

    pub fn entry_hash(key: &[u8]) -> EntryHash {
        assert!(key.len() < 32);
        let bytes: Vec<u8> = key
            .iter()
            .chain(std::iter::repeat(&0u8))
            .take(32)
            .cloned()
            .collect();

        EntryHash::try_from(bytes).unwrap()
    }

    pub fn blob_serialized(value: Vec<u8>) -> Vec<u8> {
        bincode::serialize(&blob(value)).unwrap()
    }

    pub fn all_kv_stores(
        _base_dir: PathBuf, // TODO - TE-261: not used anymore now
    ) -> HashMap<SupportedContextKeyValueStore, TestContextKvStoreFactoryInstance> {
        let mut store_factories: HashMap<
            SupportedContextKeyValueStore,
            TestContextKvStoreFactoryInstance,
        > = HashMap::new();

        for sckvs in SupportedContextKeyValueStore::iter() {
            let _ = match sckvs {
                SupportedContextKeyValueStore::InMem => store_factories.insert(
                    SupportedContextKeyValueStore::InMem,
                    Box::new(InMemoryBackendTestContextKvStoreFactory),
                ),
            };
        }

        assert_eq!(
            SupportedContextKeyValueStore::iter().count(),
            store_factories.len(),
            "There must be registered test factory for every supported kv-store!"
        );

        store_factories
    }

    pub trait TestContextKvStoreFactory: 'static + Send + Sync + Persistable {
        /// Creates new storage and also clean all existing data
        fn create(&self, name: &str) -> Result<Box<ContextKeyValueStore>, TestKeyValueStoreError>;
    }

    /// In-memory kv-store
    pub struct InMemoryBackendTestContextKvStoreFactory;

    impl TestContextKvStoreFactory for InMemoryBackendTestContextKvStoreFactory {
        fn create(&self, _: &str) -> Result<Box<ContextKeyValueStore>, TestKeyValueStoreError> {
            use crate::kv_store::in_memory::InMemory;
            Ok(Box::new(InMemory::new()))
        }
    }

    impl Persistable for InMemoryBackendTestContextKvStoreFactory {
        fn is_persistent(&self) -> bool {
            false
        }
    }
}
