use crate::storage_backend::{StorageBackend, Batch, StorageBackendError};
use sled::IVec;

pub struct SledBackend {
    inner: sled::Tree
}

impl SledBackend {
    pub fn new(db : sled::Tree) -> Self{
        SledBackend {
            inner: db
        }
    }
}




impl StorageBackend for SledBackend {
    fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        self.inner.insert(key, value)?;
        Ok(())
    }

    fn merge(&self, key: Vec<u8>, value: Vec<u8>) -> Result<(), StorageBackendError> {
        self.inner.insert(key, value)?;
        Ok(())
    }

    fn delete(&self, key: &Vec<u8>) -> Result<(), StorageBackendError> {
        self.inner.remove(key)?;
        Ok(())
    }

    fn batch_write(&self, batch: Batch) -> Result<(), StorageBackendError> {
        let mut wb = sled::Batch::default();
        for (k, v) in batch.iter() {
            wb.insert(k.clone(), v.clone());
        }
        self.inner.apply_batch(wb)?;
        Ok(())
    }

    fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>, StorageBackendError> {
        let r = self.inner.get(key)?;

        match r {
            None => {
                Err(StorageBackendError::BackendError)
            }
            Some(v) => {
                Ok(Some(v.to_vec()))
            }
        }
    }
}