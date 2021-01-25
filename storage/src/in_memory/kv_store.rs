use std::collections::{BTreeMap, btree_map::Entry};

use crate::kv_store::{
    BasicWriteBatch, BasicWriteBatchOp,
    KVStore as KVStoreTrait, ApplyBatch,
    KVStoreError};


/// In Memory Key Value Store implemented with [BTreeMap](std::collections::BTreeMap)
#[derive(Debug)]
pub struct KVStore<K: Ord, V> {
    kv_map: BTreeMap<K, V>,
}

impl<K: Ord, V> Default for KVStore<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> KVStoreTrait for KVStore<K, V>
where K: Ord,
      V: Clone,
{
    type Error = KVStoreError;
    type Key = K;
    type Value = V;

    #[inline]
    fn is_persisted(&self) -> bool { false }

    /// put kv in map if key doesn't exist. If it does then return false.
    fn put(&mut self, key: Self::Key, value: Self::Value) -> Result<bool, Self::Error> {
        match self.kv_map.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(true)
            },
            // _ => Err(KVStoreError::EntryOccupied),
            _ => Ok(false)
        }
    }

    fn merge(&mut self, key: Self::Key, value: Self::Value) -> Result<Option<Self::Value>, Self::Error> {
        Ok(self.kv_map.insert(key, value))
    }

    fn delete(&mut self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        Ok(self.kv_map.remove(key))
    }

    fn get(&self, key: &Self::Key) -> Result<Option<Self::Value>, Self::Error> {
        Ok(self.kv_map.get(key).cloned())
    }

    fn contains(&self, key: &Self::Key) -> Result<bool, Self::Error> {
        Ok(self.kv_map.contains_key(key))
    }

    fn len(&self) -> Result<usize, Self::Error> {
        Ok(self.kv_map.len())
    }
}

impl<K, V> ApplyBatch<BasicWriteBatch<K, V>, KVStoreError> for KVStore<K, V>
where K: Ord + Clone,
      V: Clone,
{
    fn apply_batch(&mut self, batch: BasicWriteBatch<K, V>) -> Result<(), KVStoreError> {
        if batch.is_empty() {
            return Ok(());
        }

        let result = batch.into_iter()
            .map(|op| {
                Ok(match op {
                    BasicWriteBatchOp::Put { key, value } => (
                        key.clone(),
                        self.put(key, value).and(Ok(None))?,
                    ),
                    BasicWriteBatchOp::Merge { key, value } => (
                        key.clone(),
                        self.merge(key, value)?,
                    ),
                    BasicWriteBatchOp::Delete { key } => {
                        let deleted = self.delete(&key)?;
                        (key, deleted)
                    },
                })
            })
            .try_fold(BTreeMap::new(), |mut changes, op_result| {
                match op_result {
                    Ok((key, value)) => {
                        // preserve only first change to the key, which we
                        // revert in case batch fails.
                        if let Entry::Vacant(entry) = changes.entry(key) {
                            entry.insert(value);
                        }
                        Ok(changes)
                    },
                    Err(err) => Err((err, changes)),
                }
            });

        // rollback if batch failed
        if let Err((err, changes)) = result {
            self.rollback_changes(changes)
                .expect("failed to roll back `WriteBatch` changes in `KVStore`");
            return Err(err);
        }
        Ok(())
    }
}


impl<K: Ord, V> KVStore<K, V> {
    pub fn new() -> Self {
        Self {
            kv_map: BTreeMap::new(),
        }
    }
}

impl<K: Ord, V: Clone> KVStore<K, V> {
    fn rollback_changes(
        &mut self,
        prev_values: BTreeMap<K, Option<V>>,
    ) -> Result<(), KVStoreError> {
        for (key, value) in prev_values.into_iter() {
            match value {
                Some(value) => self.merge(key, value)?,
                None => self.delete(&key)?,
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // #[test]
    // fn put_on_existing_key_fails() {
    //     let mut kv_store = KVStore::new();
    //     assert!(matches!(kv_store.put("a", 1), Ok(_)));
    //     assert!(matches!(kv_store.put("a", 2), Err(KVStoreError::EntryOccupied)));
    //     assert!(matches!(kv_store.get(&"a"), Ok(Some(1))));
    // }

    // #[test]
    // fn same_key_multiple_batch_ops_rolls_back_to_initial_value() {
    //     let mut kv_store = KVStore::new();

    //     let mut wt_batch = BasicWriteBatchOp::new();
    //     wt_batch.put("a", 1);
    //     wt_batch.merge("a", 2);
    //     wt_batch.put("a", 3); // should fail

    //     assert!(matches!(kv_store.write_batch(wt_batch), Err(KVStoreError::EntryOccupied)));
    //     assert!(matches!(kv_store.get(&"a"), Ok(None)));
    // }
}
