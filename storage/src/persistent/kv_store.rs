use std::marker::PhantomData;
use sled::transaction::{TransactionError, ConflictableTransactionError};

use crate::persistent::{Codec, SchemaError};
use crate::kv_store::{
    BasicWriteBatch, BasicWriteBatchOp,
    KVStore as KVStoreTrait, ApplyBatch,
    KVStoreError};


/// Key Value Store implemented with [sled]
#[derive(Debug)]
pub struct KVStore<K, V> {
    db: sled::Tree,
    _kv: PhantomData<(K, V)>
}

impl<K, V> KVStoreTrait for KVStore<K, V>
where K: Codec,
      V: Codec,
{
    type Error = KVStoreError;
    type Key = K;
    type Value = V;

    #[inline]
    fn is_persisted(&self) -> bool { true }

    fn put(&mut self, key: Self::Key, value: Self::Value) -> Result<(), Self::Error> {
        self.db.compare_and_swap(
            key.encode()?,
            None as Option<Vec<_>>,
            Some(value.encode()?)
        )?;
        Ok(())
    }

    fn merge(&mut self, key: Self::Key, value: Self::Value) -> Result<Option<Self::Value>, Self::Error> {
        Ok(self.db.insert(key.encode()?, value.encode()?)?
            .map(|v| Self::Value::decode(&v))
            .transpose()?)
    }

    fn delete(&mut self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        Ok(self.db.remove(key.encode()?)?
            .map(|v| Self::Value::decode(&v))
            .transpose()?)
    }

    fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        Ok(self.db.get(key.encode()?)?
            .map(|v| Self::Value::decode(&v))
            .transpose()?)
    }

    fn contains(&self, key: &Self::Key) -> Result<bool, Self::Error> {
        Ok(self.db.contains_key(key.encode()?)?)
    }

    fn len(&self) -> Result<usize, Self::Error> {
        Ok(self.db.len())
    }
}

impl<K, V> ApplyBatch<BasicWriteBatch<K, V>, KVStoreError> for KVStore<K, V>
where K: Codec,
      V: Codec,
{
    fn apply_batch(&mut self, batch: BasicWriteBatch<K, V>) -> Result<(), KVStoreError> {
        if batch.is_empty() {
            return Ok(());
        }

        // self.db.apply_batch(batch) - can't be used since [sled::Batch]
        // doesn't support put operation (which doesn't overwrite key if it exists).
        self.db.transaction(|tx| {
            for op in batch.iter() {
                match op {
                    BasicWriteBatchOp::Put { key, value } => {
                        let (key, value) = (key.encode()?, value.encode()?);
                        tx.get(key.clone()).and_then(|_| tx.insert(key, value))?;
                    },
                    BasicWriteBatchOp::Merge { key, value } => {
                        tx.insert(key.encode()?, value.encode()?)?;
                    },
                    BasicWriteBatchOp::Delete { key } => {
                        tx.remove(key.encode()?)?;
                    }
                }
            }
            Ok(())
        }).map_err(|error| -> KVStoreError {
            match error {
                TransactionError::Abort(err) => err.into(),
                TransactionError::Storage(err) => TransactionError::Storage(err).into(),
            }
        })?;

        Ok(())
    }
}

impl<K, V> KVStore<K, V> {
    pub fn new(db: sled::Tree) -> Self {
        Self { db, _kv: PhantomData {} }
    }
}

impl From<SchemaError> for TransactionError<SchemaError> {
    fn from(error: SchemaError) -> Self {
        TransactionError::Abort(error)
    }
}

impl From<SchemaError> for ConflictableTransactionError<SchemaError> {
    fn from(error: SchemaError) -> Self {
        ConflictableTransactionError::Abort(error)
    }
}
