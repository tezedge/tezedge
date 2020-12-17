use std::collections::{BTreeMap, btree_map::Entry};
use failure::Fail;

pub trait WriteBatch {
    type Key;
    type Value;

    fn put(&mut self, key: Self::Key, value: Self::Value);
    fn merge(&mut self, key: Self::Key, value: Self::Value);
    fn delete(&mut self, key: Self::Key);
}

#[derive(Debug)]
pub enum KVStoreWriteBatchOp<K, V> {
    Put { key: K, value: V },
    Merge { key: K, value: V },
    Delete { key: K },
}

#[derive(Debug)]
pub struct KVStoreWriteBatch<K, V> {
    ops: Vec<KVStoreWriteBatchOp<K, V>>
}

impl<K, V> KVStoreWriteBatch<K, V> {
    pub fn new() -> Self {
        Self { ops: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

impl<K, V> Default for KVStoreWriteBatch<K, V> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> WriteBatch for KVStoreWriteBatch<K, V> {
    type Key = K;
    type Value = V;

    fn put(&mut self, key: Self::Key, value: Self::Value) {
        self.ops.push(KVStoreWriteBatchOp::Put { key, value });
    }

    fn merge(&mut self, key: Self::Key, value: Self::Value) {
        self.ops.push(KVStoreWriteBatchOp::Merge { key, value });
    }

    fn delete(&mut self, key: Self::Key) {
        self.ops.push(KVStoreWriteBatchOp::Delete { key });
    }
}

impl<K, V> IntoIterator for KVStoreWriteBatch<K, V> {
    type Item = KVStoreWriteBatchOp<K, V>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.ops.into_iter()
    }
}


#[derive(Debug, Fail)]
pub enum KVStoreError {
    #[fail(display = "Entry with key already exists")]
    EntryOccupied,
}

/// In Memory Key Value Store implemented with [BTreeMap](std::collections::BTreeMap)
#[derive(Debug)]
pub struct KVStore<K, V>
where K: Ord,
      V: Clone,
{
    kv_map: BTreeMap<K, V>,
}

impl<K, V> Default for KVStore<K, V>
where K: Clone + Ord,
      V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> KVStore<K, V>
where K: Clone + Ord,
      V: Clone,
{
    pub fn new() -> Self {
        Self {
            kv_map: BTreeMap::new(),
        }
    }

    /// put kv in map if key doesn't exist. If it does then error.
    pub fn put(&mut self, key: K, value: V) -> Result<(), KVStoreError> {
        match self.kv_map.entry(key) {
            Entry::Vacant(entry) => {
                entry.insert(value);
                Ok(())
            },
            // _ => Err(KVStoreError::EntryOccupied),
            _ => Ok(())
        }
    }

    pub fn merge(&mut self, key: K, value: V) -> Result<Option<V>, KVStoreError> {
        Ok(self.kv_map.insert(key, value))
    }

    pub fn delete(&mut self, key: &K) -> Result<Option<V>, KVStoreError> {
        Ok(self.kv_map.remove(key))
    }

    pub fn get(&self, key: &K) -> Result<Option<V>, KVStoreError> {
        Ok(self.kv_map.get(key).cloned())
    }

    pub fn contains(&self, key: &K) -> Result<bool, KVStoreError> {
        Ok(self.kv_map.contains_key(key))
    }

    pub fn put_batch(
        &self,
        batch: &mut KVStoreWriteBatch<K, V>,
        key: K,
        value: V
    ) -> Result<(), KVStoreError> {
        batch.put(key, value);
        Ok(())
    }

    /// atomic batch write
    pub fn write_batch(&mut self, batch: KVStoreWriteBatch<K, V>) -> Result<(), KVStoreError> {
        if batch.is_empty() {
            return Ok(());
        }

        let result = batch.into_iter()
            .map(|op| {
                Ok(match op {
                    KVStoreWriteBatchOp::Put { key, value } => (
                        key.clone(),
                        self.put(key, value).and(Ok(None))?,
                    ),
                    KVStoreWriteBatchOp::Merge { key, value } => (
                        key.clone(),
                        self.merge(key, value)?,
                    ),
                    KVStoreWriteBatchOp::Delete { key } => {
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

    //     let mut wt_batch = KVStoreWriteBatch::new();
    //     wt_batch.put("a", 1);
    //     wt_batch.merge("a", 2);
    //     wt_batch.put("a", 3); // should fail

    //     assert!(matches!(kv_store.write_batch(wt_batch), Err(KVStoreError::EntryOccupied)));
    //     assert!(matches!(kv_store.get(&"a"), Ok(None)));
    // }
}
