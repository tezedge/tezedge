use std::{cell::RefCell, collections::HashMap, path::PathBuf, sync::Mutex, unimplemented};

use crate::persistent::database::{IteratorMode, IteratorWithSchema, RocksDBStats};
use crate::persistent::{DBError, KeyValueSchema, KeyValueStoreWithSchema};

use super::Codec;
use crate::persistent::codec::{Decoder, Encoder};
use sled;

pub struct InMemoryStorage<S: KeyValueSchema> {
    in_memory_data: Mutex<RefCell<HashMap<S::Key, S::Value>>>,
    path: PathBuf,
}

impl<S: KeyValueSchema> InMemoryStorage<S>
where
    S::Key: Codec + std::cmp::Eq + std::hash::Hash,
    S::Value: Codec,
{
    pub fn new(path: PathBuf) -> InMemoryStorage<S> {
        let storage: InMemoryStorage<S> = InMemoryStorage {
            in_memory_data: Mutex::new(RefCell::new(HashMap::new())),
            path: path.clone(),
        };

        match sled::open(&storage.path) {
            Ok(db) => {
                for i in db.iter() {
                    let data = i.expect("cannot read entry from db");
                    let key = S::Key::decode(&data.0).unwrap();
                    let val = S::Value::decode(&data.1).unwrap();
                    storage
                        .in_memory_data
                        .lock()
                        .unwrap()
                        .borrow_mut()
                        .insert(key, val);
                }
                storage
            }
            Err(_) => storage,
        }
    }
}

impl<S: KeyValueSchema> Drop for InMemoryStorage<S> {
    fn drop(&mut self) {
        let db = sled::Config::default().path(&self.path).open().unwrap();
        db.clear().expect("cannot clear file storage");
        for (k, v) in self.in_memory_data.lock().unwrap().borrow().iter() {
            let key_bytes = k.encode().expect("cannot serialize key");
            let value_bytes = v.encode().expect("cannot deserialize key");
            db.insert(key_bytes, value_bytes)
                .expect("cannot store data in psersitent storage");
        }
    }
}

// currently only part of the interface is used for MerkleStorage use case
// where we want to use InMemoryStorage therefore part of the methods can be
// implemented later
impl<S> KeyValueStoreWithSchema<S> for InMemoryStorage<S>
where
    S: KeyValueSchema,
    S::Key: std::cmp::Eq + std::hash::Hash,
    S::Value: Clone,
{
    fn put(&self, _key: &S::Key, _value: &S::Value) -> Result<(), DBError> {
        unimplemented!("Foo::put not implemented yet");
    }

    fn delete(&self, _key: &S::Key) -> Result<(), DBError> {
        unimplemented!("Foo::delete not implemented yet");
    }

    fn merge(&self, _key: &S::Key, _value: &S::Value) -> Result<(), DBError> {
        unimplemented!("Foo::merge not implemented yet");
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, DBError> {
        Ok(self
            .in_memory_data
            .lock()
            .unwrap()
            .borrow()
            .get(key)
            .map(|x| x.clone()))
    }

    fn iterator(&self, _mode: IteratorMode<S>) -> Result<IteratorWithSchema<S>, DBError> {
        unimplemented!("Foo::iterator not implemented yet");
    }

    fn prefix_iterator(&self, _key: &S::Key) -> Result<IteratorWithSchema<S>, DBError> {
        unimplemented!("Foo::prefix_iterator not implemented yet");
    }

    fn contains(&self, _key: &S::Key) -> Result<bool, DBError> {
        unimplemented!("Foo::contains not implemented yet");
    }

    fn write(&self, batch: Vec<(S::Key, S::Value)>) -> Result<(), DBError> {
        for (k, v) in batch.into_iter() {
            self.in_memory_data
                .lock()
                .unwrap()
                .borrow_mut()
                .insert(k, v);
        }
        return Ok(());
    }

    fn get_mem_use_stats(&self) -> Result<RocksDBStats, DBError> {
        unimplemented!("Foo::get_mem_use_stats not implemented yet");
    }
}
