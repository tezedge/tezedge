use crate::storage_backend::{Batch, StorageBackend, StorageBackendError};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, RwLock};

pub struct InMemoryBackend {
    inner: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl InMemoryBackend {
    pub fn new(db: Arc<RwLock<HashMap<Vec<u8>, Vec<u8>>>>) -> Self {
        InMemoryBackend { inner: db }
    }
}

impl StorageBackend for InMemoryBackend {
    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        let mut w =
            self.inner
                .write()
                .map(|w| w)
                .map_err(|e| StorageBackendError::GuardPoison {
                    error: format!("{}", e),
                })?;

        w.insert(key, value);
        Ok(())
    }

    fn merge(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        let mut w =
            self.inner
                .write()
                .map(|w| w)
                .map_err(|e| StorageBackendError::GuardPoison {
                    error: format!("{}", e),
                })?;

        w.insert(key, value);
        Ok(())
    }

    fn delete(&self, key: &Vec<u8>) -> Result<(), StorageBackendError> {
        let mut w =
            self.inner
                .write()
                .map(|w| w)
                .map_err(|e| StorageBackendError::GuardPoison {
                    error: format!("{}", e),
                })?;

        w.remove(key);
        Ok(())
    }

    fn batch_write(&self, batch: Batch) -> Result<(), StorageBackendError> {
        let mut w =
            self.inner
                .write()
                .map(|w| w)
                .map_err(|e| StorageBackendError::GuardPoison {
                    error: format!("{}", e),
                })?;

        for (key, value) in batch.iter() {
            w.insert(key.clone(), value.clone());
        }
        Ok(())
    }

    fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>, StorageBackendError> {
        let db = self.inner.clone();
        let mut r = db
            .read()
            .map(|r| r)
            .map_err(|e| StorageBackendError::GuardPoison {
                error: format!("{}", e),
            })?;

        match r.get(key) {
            None => Err(StorageBackendError::BackendError),
            Some(v) => Ok(Some(v.clone())),
        }
    }
}
