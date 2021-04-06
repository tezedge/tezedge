use im::HashMap;
use std::sync::Arc;
use sled::Tree;
use std::path::Path;
use crate::database::error::Error;
use crate::database::{KVDatabase, DBSubtreeKeyValueSchema};
use crate::persistent::{KeyValueSchema, Encoder, Decoder};
use std::alloc::Global;

pub struct DB {
    inner: Arc<HashMap<String, Tree>>
}

impl DB {
    pub fn initialize<P: AsRef<Path>>(db_path: P, subs_trees: Vec<String>) -> Result<Self, Error> {
        let db = sled::Config::new()
            .path(db_path)
            .use_compression(true)
            .mode(sled::Mode::LowSpace)
            .cache_capacity(200_000_000)
            .open()
            .map_err(Error::from)?;


        let mut trees = HashMap::new();

        for name in subs_trees {
            trees.insert(name.to_owned(), db.open_tree(name.as_str()).map_err(Error::from)?);
        }

        let db = DB {
            inner: Arc::new(trees)
        };
        Ok(db)
    }

    fn get_tree(&self, name: &str) -> Result<Tree, Error> {
        let tree = match self.inner.get(name) {
            None => {
                return Err(Error::MissingSubTree { error: name.to_owned() });
            }
            Some(t) => {
                t
            }
        };
        Ok(tree.clone())
    }
}

impl<S: DBSubtreeKeyValueSchema> KVDatabase<S> for DB {
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
        let _ = tree.insert(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, Error> {
        let key = key.encode()?;
        let tree = self.get_tree(S::sub_tree_name())?;
        tree.get(key).map_err(Error::from)?.map(|value|S::Value::decode(value.as_ref()))
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
            sled_batch.insert(key,value);
        }

        let _ = tree.apply_batch(sled_batch).map_err(Error::from)?;
        Ok(())
    }
}