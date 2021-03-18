// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This sub module provides different KV alternatives for context persistence

use std::path::PathBuf;
use std::str::FromStr;

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

pub mod btree_map;
pub mod in_memory_backend;
pub mod rocksdb_backend;
pub mod sled_backend;
pub mod storage_backend;

pub const ROCKSDB: &str = "rocksdb";

#[derive(PartialEq, Eq, Hash, Debug, Clone, EnumIter)]
pub enum SupportedContextKeyValueStore {
    RocksDB { path: PathBuf },
    InMem,
    Sled { path: PathBuf },
    BTreeMap,
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
            SupportedContextKeyValueStore::RocksDB { .. } => vec![ROCKSDB],
            SupportedContextKeyValueStore::InMem => vec!["inmem"],
            SupportedContextKeyValueStore::Sled { .. } => vec!["sled"],
            SupportedContextKeyValueStore::BTreeMap => vec!["btree"],
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
    extern crate proc_macro;

    use std::collections::HashMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use strum::IntoEnumIterator;

    use crate::context::merkle::merkle_storage::MerkleStorageKV;
    use crate::persistent::database::RocksDbKeyValueSchema;

    use super::SupportedContextKeyValueStore;

    pub type TestKeyValueStoreError = failure::Error;
    pub type TestContextKvStoreFactoryInstance = Box<dyn TestContextKvStoreFactory>;

    pub fn all_kv_stores(
        base_dir: PathBuf,
    ) -> HashMap<SupportedContextKeyValueStore, TestContextKvStoreFactoryInstance> {
        let mut store_factories: HashMap<
            SupportedContextKeyValueStore,
            TestContextKvStoreFactoryInstance,
        > = HashMap::new();

        for sckvs in SupportedContextKeyValueStore::iter() {
            let _ = match sckvs {
                SupportedContextKeyValueStore::RocksDB { .. } => store_factories.insert(
                    SupportedContextKeyValueStore::RocksDB {
                        path: base_dir.clone(),
                    },
                    Box::new(RocksDbBackendTestContextKvStoreFactory {
                        base_path: base_dir.clone(),
                    }),
                ),
                SupportedContextKeyValueStore::InMem => store_factories.insert(
                    SupportedContextKeyValueStore::InMem,
                    Box::new(InMemoryBackendTestContextKvStoreFactory),
                ),
                SupportedContextKeyValueStore::Sled { .. } => store_factories.insert(
                    SupportedContextKeyValueStore::Sled {
                        path: base_dir.clone(),
                    },
                    Box::new(SledBackendTestContextKvStoreFactory {
                        base_path: base_dir.clone(),
                    }),
                ),
                SupportedContextKeyValueStore::BTreeMap => store_factories.insert(
                    SupportedContextKeyValueStore::BTreeMap,
                    Box::new(BTreeMapBackendTestContextKvStoreFactory),
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

    pub trait TestContextKvStoreFactory: 'static + Send + Sync {
        /// Creates new storage and also clean all existing data
        fn create(&self, name: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError>;

        // TODO: open new instance

        /// Just opens a storage, does not clean anything
        fn open(&self, name: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError>;
    }

    /// In-memory kv-store
    pub struct InMemoryBackendTestContextKvStoreFactory;

    impl TestContextKvStoreFactory for InMemoryBackendTestContextKvStoreFactory {
        fn create(&self, _: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::in_memory_backend::InMemoryBackend;
            Ok(Box::new(InMemoryBackend::new()))
        }

        fn open(&self, _: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::in_memory_backend::InMemoryBackend;
            Ok(Box::new(InMemoryBackend::new()))
        }
    }

    /// BTree map kv-store
    pub struct BTreeMapBackendTestContextKvStoreFactory;

    impl TestContextKvStoreFactory for BTreeMapBackendTestContextKvStoreFactory {
        fn create(&self, _: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::btree_map::BTreeMapBackend;
            Ok(Box::new(BTreeMapBackend::new()))
        }

        fn open(&self, _: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::btree_map::BTreeMapBackend;
            Ok(Box::new(BTreeMapBackend::new()))
        }
    }

    /// Sled map kv-store
    pub struct SledBackendTestContextKvStoreFactory {
        base_path: PathBuf,
    }

    impl SledBackendTestContextKvStoreFactory {
        fn db_path(&self, name: &str) -> PathBuf {
            self.base_path.join(format!("sled_{}", name))
        }

        fn db(&self, db_name: &str, create_new: bool) -> Result<sled::Db, TestKeyValueStoreError> {
            Ok(sled::Config::new()
                .path(self.db_path(db_name))
                .create_new(create_new)
                .open()?)
        }
    }

    impl TestContextKvStoreFactory for SledBackendTestContextKvStoreFactory {
        fn create(&self, name: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::sled_backend::SledBackend;

            // clear files
            let db_path = self.db_path(name);
            if Path::new(&db_path).exists() {
                let _ = fs::remove_dir_all(&db_path)?;
            }

            // open and clear db
            let db = self.db(name, true)?;
            db.clear()?;
            db.flush()?;

            Ok(Box::new(SledBackend::new(db)))
        }

        fn open(&self, name: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::sled_backend::SledBackend;

            // just open db
            let db = self.db(name, false)?;
            Ok(Box::new(SledBackend::new(db)))
        }
    }

    /// Rocksdb map kv-store
    pub struct RocksDbBackendTestContextKvStoreFactory {
        base_path: PathBuf,
    }

    impl RocksDbBackendTestContextKvStoreFactory {
        fn db_path(&self, name: &str) -> PathBuf {
            self.base_path.join(format!("rockdb_{}", name))
        }

        fn db(
            &self,
            db_name: &str,
            create_new: bool,
        ) -> Result<rocksdb::DB, TestKeyValueStoreError> {
            use crate::context::kv_store::rocksdb_backend::RocksDBBackend;
            use rocksdb::{Cache, Options, DB};

            let mut db_opts = Options::default();
            db_opts.create_if_missing(create_new);
            db_opts.create_missing_column_families(create_new);

            let cache =
                Cache::new_lru_cache(32 * 1024 * 1024).expect("Failed to create rocksdb cache");

            Ok(DB::open_cf_descriptors(
                &db_opts,
                self.db_path(db_name),
                vec![RocksDBBackend::descriptor(&cache)],
            )?)
        }
    }

    impl TestContextKvStoreFactory for RocksDbBackendTestContextKvStoreFactory {
        fn create(&self, name: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::rocksdb_backend::RocksDBBackend;

            // clear files
            let db_path = self.db_path(name);
            if Path::new(&db_path).exists() {
                let _ = fs::remove_dir_all(&db_path)?;
            }

            // open and clear db
            let db = self.db(name, true)?;
            db.flush()?;

            Ok(Box::new(RocksDBBackend::new(Arc::new(db))))
        }

        fn open(&self, name: &str) -> Result<Box<MerkleStorageKV>, TestKeyValueStoreError> {
            use crate::context::kv_store::rocksdb_backend::RocksDBBackend;

            // just open db
            let db = self.db(name, false)?;
            Ok(Box::new(RocksDBBackend::new(Arc::new(db))))
        }
    }
}
