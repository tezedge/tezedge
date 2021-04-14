use crate::database::error::Error;
use crate::database::{
    DBSubtreeKeyValueSchema, KVDBIteratorWithSchema, KVDBStoreWithSchema, KVDatabase,
    KVDatabaseWithSchemaIterator, SledIteratorWrapper, SledIteratorWrapperMode,
};
use crate::persistent::{Decoder, Encoder, KeyValueSchema};
use crate::IteratorMode;
use im::HashMap;
use sled::{IVec, Tree};
use std::alloc::Global;
use std::path::Path;
use std::sync::Arc;

use std::marker::PhantomData;

pub struct MainDB {
    inner: Arc<HashMap<String, Arc<Tree>>>,
}

fn replace_merge(
    _key: &[u8],               // the key being merged
    _old_value: Option<&[u8]>,  // the previous value, if one existed
    merged_bytes: &[u8]        // the new bytes being merged in
) -> Option<Vec<u8>> {       // set the new value, return None to delete
    Some(merged_bytes.to_vec())
}

impl MainDB {
    pub fn initialize<P: AsRef<Path>>(
        db_path: P,
        trees: Vec<String>,
        is_temporary: bool,
    ) -> Result<Self, Error> {
        let db = sled::Config::new()
            .path(db_path)
            .temporary(is_temporary)
            .use_compression(true)
            .mode(sled::Mode::LowSpace)
            .cache_capacity(200_000_000)
            .open()
            .map_err(Error::from)?;

        let mut tree_map = HashMap::new();

        for name in trees {
            let tree = db.open_tree(name.as_str()).map_err(Error::from)?;
            tree.set_merge_operator(replace_merge);
            tree_map.insert(
                name.to_owned(),
                Arc::new(tree)
            );
        }

        let db = MainDB {
            inner: Arc::new(tree_map),
        };
        Ok(db)
    }

    fn get_tree(&self, name: &'static str) -> Result<Arc<Tree>, Error> {
        let tree = match self.inner.get(name) {
            None => {
                return Err(Error::MissingSubTree {
                    error: name.to_owned(),
                });
            }
            Some(t) => t,
        };
        Ok(tree.clone())
    }

    fn tree_insert(&self, name : &str, key : Vec<u8>, value: Vec<u8> ) -> Result<(), Error>  {
        let tree = match self.inner.get(name) {
            None => {
                return Err(Error::MissingSubTree {
                    error: name.to_owned(),
                });
            }
            Some(t) => t,
        };
        let _ = tree.insert(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn tree_get(&self, name : &str, key : Vec<u8>) -> Result<Option<IVec>, Error>  {
        let tree = match self.inner.get(name) {
            None => {
                return Err(Error::MissingSubTree {
                    error: name.to_owned(),
                });
            }
            Some(t) => t,
        };
        tree.get(key).map_err(Error::from)
    }

    fn tree_delete(&self, name : &str, key : Vec<u8>) -> Result<Option<IVec>, Error>  {
        let tree = match self.inner.get(name) {
            None => {
                return Err(Error::MissingSubTree {
                    error: name.to_owned(),
                });
            }
            Some(t) => t,
        };
        tree.remove(key).map_err(Error::from)
    }

    fn tree_merge(&self, name : &str, key : Vec<u8>, value: Vec<u8>) -> Result<Option<IVec>, Error>  {
        let tree = match self.inner.get(name) {
            None => {
                return Err(Error::MissingSubTree {
                    error: name.to_owned(),
                });
            }
            Some(t) => t,
        };
        tree.merge(key,value).map_err(Error::from)
    }

    pub fn flush(&self) -> Result<(), Error> {
        for tree in self.inner.values() {
            match tree.flush() {
                Ok(_) => {}
                Err(error) => {
                    println!("Flush failed {}", error)
                }
            }
        }
        Ok(())
    }
}

impl<S: DBSubtreeKeyValueSchema> KVDatabase<S> for MainDB {
    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), Error> {
        let key = key.encode()?;
        let value = value.encode()?;
        let tree = self.get_tree(S::sub_tree_name())?;
        let _ = tree.insert(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn delete(&self, key: &S::Key) -> Result<(), Error> {
        let key = key.encode()?;
        let tree = self.get_tree(S::sub_tree_name())?;
        let _ = tree.remove(key).map_err(Error::from)?;
        Ok(())
    }

    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), Error> {
        let key = key.encode()?;
        let value = value.encode()?;
        let tree = self.get_tree(S::sub_tree_name())?;
        let _ = tree.merge(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, Error> {
        let key = key.encode()?;
        let tree = self.get_tree(S::sub_tree_name())?;
        tree.get(key)
            .map_err(Error::from)?
            .map(|value| S::Value::decode(value.as_ref()))
            .transpose()
            .map_err(Error::from)
    }

    fn contains(&self, key: &S::Key) -> Result<bool, Error> {
        let key = key.encode()?;
        let tree = self.get_tree(S::sub_tree_name())?;
        let value = tree.get(key).map_err(Error::from)?;
        Ok(value.is_some())
    }

    fn write_batch(&self, batch: Vec<(S::Key, S::Value), Global>) -> Result<(), Error> {
        let tree = self.get_tree(S::sub_tree_name())?;
        let mut sled_batch = sled::Batch::default();

        for (k, v) in batch.iter() {
            let key = k.encode()?;
            let value = v.encode()?;
            sled_batch.insert(key, value);
        }

        let _ = tree.apply_batch(sled_batch).map_err(Error::from)?;
        Ok(())
    }
}
impl<S: DBSubtreeKeyValueSchema> KVDBStoreWithSchema<S> for MainDB {}

impl<S: DBSubtreeKeyValueSchema> KVDatabaseWithSchemaIterator<S> for MainDB {
    fn iterator(&self, mode: IteratorMode<S>) -> Result<KVDBIteratorWithSchema<S>, Error> {
        let tree = self.get_tree(S::sub_tree_name())?;

        let iter = match mode {
            IteratorMode::Start => SledIteratorWrapper::new(SledIteratorWrapperMode::Start, tree),
            IteratorMode::End => SledIteratorWrapper::new(SledIteratorWrapperMode::End, tree),
            IteratorMode::From(key, direction) => SledIteratorWrapper::new(
                SledIteratorWrapperMode::From(IVec::from(key.encode()?), direction),
                tree,
            ),
        };
        Ok(KVDBIteratorWithSchema(iter, PhantomData))
    }

    fn prefix_iterator(
        &self,
        key: &<S as KeyValueSchema>::Key,
        max_key_len: usize,
    ) -> Result<KVDBIteratorWithSchema<S>, Error> {
        let tree = self.get_tree(S::sub_tree_name())?;
        let key = key.encode()?;
        let prefix_key = key[..max_key_len].to_vec();
        let iter = SledIteratorWrapper::new(
            SledIteratorWrapperMode::Prefix(IVec::from(prefix_key)),
            tree,
        );
        Ok(KVDBIteratorWithSchema(iter, PhantomData))
    }
}
