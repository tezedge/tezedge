// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, hash_map::Entry, HashSet};
use std::sync::{Arc,RwLock, Mutex};

use rocksdb::WriteBatch;
use crate::merkle_storage::{ContextValue, EntryHash};
use crate::persistent::database::{KeyValueStoreWithSchemaIterator, SimpleKeyValueStoreWithSchema, DBError, RocksDBStats, IteratorMode, IteratorWithSchema};
use crate::persistent::schema::KeyValueSchema;
use crate::MerkleStorage;
use std::ops::{DerefMut, AddAssign, SubAssign};
use crate::storage_backend::{StorageBackend, StorageBackendError, StorageBackendStats};

#[derive(Default)]
pub struct InMemoryBackend {
    inner: Arc<RwLock<HashMap<EntryHash, ContextValue>>>,
    stats: Mutex<StorageBackendStats>,
}

impl InMemoryBackend {
    pub fn new() -> Self {
        InMemoryBackend {
            inner: Arc::new(RwLock::new(HashMap::new())),
            stats: Default::default(),
        }
    }
}

impl SimpleKeyValueStoreWithSchema<MerkleStorage> for InMemoryBackend {

    fn retain(&self, predicate: &dyn Fn(&EntryHash) -> bool) -> Result<(), DBError>{
        let garbage_keys: Vec<_> = self
            .inner
            .read()
            .unwrap()
            .iter()
            .filter_map(|(k, _)| {
                if !predicate(k) {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn put(& self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        let measurement = StorageBackendStats::from((key, value));
        let mut w = self
            .inner
            .write()
            .map_err(|e| DBError::GuardPoison {
                error: format!("{}", e),
            })?;

        if w.contains_key(key){
            Err(DBError::ValueExists{key: "blah".to_string()})
        }else{
            w.insert(*key,value.clone());
            self.stats.lock().unwrap().deref_mut().add_assign(measurement);
            Ok(())
        }
    }

    fn delete(&self, key: &EntryHash) -> Result<(), DBError> {
        let mut w = self
            .inner
            .write()
            .map_err(|e| DBError::GuardPoison {
                error: format!("{}", e),
            })?;

        let removed_key = w.remove(key);

        if let Some(v) =  &removed_key{
            self.stats.lock().unwrap().deref_mut().sub_assign(StorageBackendStats::from((key, v)));
        }

        Ok(())
    }

    fn merge(&self, key: &EntryHash, value: &ContextValue) -> Result<(), DBError> {
        let measurement = StorageBackendStats::from((key, value));
        let mut w = self
            .inner
            .write()
            .map_err(|e| DBError::GuardPoison {
                error: format!("{}", e),
            })?;


        if let Some(prev) = w.insert(*key, value.clone()){
            self.stats.lock().unwrap().deref_mut().sub_assign(StorageBackendStats::from((key, &prev)));
        };
        self.stats.lock().unwrap().deref_mut().add_assign(measurement);
        Ok(())
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, DBError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;

        match r.get(key) {
            None => Ok(None),
            Some(v) => Ok(Some(v.clone())),
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, DBError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| DBError::GuardPoison {
            error: format!("{}", e),
        })?;
        Ok(r.contains_key(key))
    }

    fn put_batch(
        &self,
        batch: &mut WriteBatch,
        key: &EntryHash,
        value: &ContextValue,
    ) -> Result<(), DBError> {
        Ok(())
    }

    fn write_batch(&self, batch: WriteBatch) -> Result<(), DBError> {
        unimplemented!();
    }

    fn get_stats(&self) -> Result<RocksDBStats, DBError> {
        Ok(RocksDBStats {
                mem_table_total: 0,
                mem_table_unflushed: 0,
                mem_table_readers_total: 0,
                cache_total: 0,
            }
        )
    }
}

impl StorageBackend for InMemoryBackend {
    fn is_persisted(&self) -> bool {
        false
    }

    fn put(&mut self, key: &EntryHash, value: ContextValue) -> Result<bool, StorageBackendError> {
        let measurement = StorageBackendStats::from((key, &value));
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;

        if w.contains_key(key){
            Ok(false)
        }else{
            w.insert(*key,value);
            // self.stats += measurement;
            Ok(true)
        }
    }

    fn merge(&mut self, key: &EntryHash, value: ContextValue) -> Result<(), StorageBackendError> {
        let measurement = StorageBackendStats::from((key, &value));
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;


        if let Some(prev) = w.insert(*key, value){
            // self.stats -= StorageBackendStats::from((key, &prev));
        };
        // self.stats += measurement;
        Ok(())
    }

    fn delete(&mut self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let mut w = self
            .inner
            .write()
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;

        let removed_key = w.remove(key);

        if let Some(v) =  &removed_key{
            // self.stats -= StorageBackendStats::from((key, v));
        }

        Ok(removed_key)
    }

    fn get(&self, key: &EntryHash) -> Result<Option<ContextValue>, StorageBackendError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| StorageBackendError::GuardPoison {
            error: format!("{}", e),
        })?;

        match r.get(key) {
            None => Ok(None),
            Some(v) => Ok(Some(v.clone())),
        }
    }

    fn contains(&self, key: &EntryHash) -> Result<bool, StorageBackendError> {
        let db = self.inner.clone();
        let r = db.read().map_err(|e| StorageBackendError::GuardPoison {
            error: format!("{}", e),
        })?;
        Ok(r.contains_key(key))
    }

    fn retain(&mut self, pred: HashSet<EntryHash>) -> Result<(), StorageBackendError> {
        let garbage_keys: Vec<_> = self
            .inner
            .read()
            .unwrap()
            .iter()
            .filter_map(|(k, _)| {
                if !pred.contains(k) {
                    Some(*k)
                } else {
                    None
                }
            })
            .collect();

        for k in garbage_keys {
            self.delete(&k)?;
        }
        Ok(())
    }

    fn total_get_mem_usage(&self) -> Result<usize,StorageBackendError>{
        Ok(self.stats.lock().unwrap().total_as_bytes())
    }
}
