use crate::database::error::Error;
use crate::database::{
    DBSubtreeKeyValueSchema, KVDBIteratorWithSchema, KVDBStoreWithSchema, KVDatabase,
    KVDatabaseWithSchemaIterator, SledIteratorWrapper, SledIteratorWrapperMode,
};
use crate::persistent::{Decoder, Encoder, KeyValueSchema};
use crate::IteratorMode;
use sled::IVec;
use std::alloc::Global;
use std::marker::PhantomData;
pub struct SledKvTree {
    tree: sled::Tree,
}

impl SledKvTree {
    pub fn new(tree: sled::Tree) -> Self {
        Self { tree }
    }
}

impl<S: DBSubtreeKeyValueSchema> KVDatabase<S> for SledKvTree {
    fn put(&self, key: &S::Key, value: &S::Value) -> Result<(), Error> {
        let key = key.encode()?;
        let value = value.encode()?;
        let _ = self.tree.insert(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn delete(&self, key: &S::Key) -> Result<(), Error> {
        let key = key.encode()?;
        let _ = self.tree.remove(key).map_err(Error::from)?;
        Ok(())
    }

    fn merge(&self, key: &S::Key, value: &S::Value) -> Result<(), Error> {
        let key = key.encode()?;
        let value = value.encode()?;
        self.tree.merge(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn get(&self, key: &S::Key) -> Result<Option<S::Value>, Error> {
        let key = key.encode()?;
        self.tree
            .get(key)
            .map_err(Error::from)?
            .map(|value| S::Value::decode(value.as_ref()))
            .transpose()
            .map_err(Error::from)
    }

    fn contains(&self, key: &S::Key) -> Result<bool, Error> {
        let key = key.encode()?;
        let value = self.tree.get(key).map_err(Error::from)?;
        Ok(value.is_some())
    }

    fn write_batch(&self, batch: Vec<(S::Key, S::Value), Global>) -> Result<(), Error> {
        let mut sled_batch = sled::Batch::default();

        for (k, v) in batch.iter() {
            let key = k.encode()?;
            let value = v.encode()?;
            sled_batch.insert(key, value);
        }

        let _ = self.tree.apply_batch(sled_batch).map_err(Error::from)?;
        Ok(())
    }
}
impl<S: DBSubtreeKeyValueSchema> KVDBStoreWithSchema<S> for SledKvTree {}

impl<S: DBSubtreeKeyValueSchema> KVDatabaseWithSchemaIterator<S> for SledKvTree {
    fn iterator(&self, mode: IteratorMode<S>) -> Result<KVDBIteratorWithSchema<S>, Error> {
        let tree = self.tree.clone();

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
        let tree = self.tree.clone();
        let key = key.encode()?;
        let prefix_key = key[..max_key_len].to_vec();
        let iter = SledIteratorWrapper::new(
            SledIteratorWrapperMode::Prefix(IVec::from(prefix_key)),
            tree,
        );
        Ok(KVDBIteratorWithSchema(iter, PhantomData))
    }
}
