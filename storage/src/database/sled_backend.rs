use crate::block_meta_storage;
use crate::database::backend::{BackendIteratorMode, TezedgeDatabaseBackendStore};
use crate::database::error::Error;
use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezdegeDatabaseBackendKV};
use crate::operations_meta_storage;
use crate::persistent::SchemaError;
use crate::{BlockMetaStorage, Direction, OperationsMetaStorage};
use sled::{Config, IVec, Tree};
use std::path::Path;

pub struct SledDBBackend {
    db: sled::Db,
}

impl SledDBBackend {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let db = Config::default()
            .path(path)
            .flush_every_ms(Some(1))
            .cache_capacity(10_000)
            .mode(sled::Mode::LowSpace)
            .open()
            .map_err(Error::from)?;
        Ok(Self { db })
    }
    pub fn get_tree(&self, name: &'static str) -> Result<Tree, Error> {
        let tree = self.db.open_tree(name).map_err(Error::from)?;

        // TODO - TE-498: refactor - SledBackend should be universal, this should be pass here by "some cfg"
        if name == OperationsMetaStorage::column_name() {
            tree.set_merge_operator(operations_meta_storage::merge_meta_value_sled)
        }
        // TODO - TE-498: refactor - SledBackend should be universal, this should be pass here by "some cfg"
        if name == BlockMetaStorage::column_name() {
            tree.set_merge_operator(block_meta_storage::merge_meta_value_sled)
        }

        Ok(tree)
    }
}

impl TezedgeDatabaseBackendStore for SledDBBackend {
    fn put(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let tree = self.get_tree(column)?;
        let _ = tree.insert(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn delete(&self, column: &'static str, key: &[u8]) -> Result<(), Error> {
        let tree = self.get_tree(column)?;
        let _ = tree.remove(key).map_err(Error::from)?;
        Ok(())
    }

    fn merge(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let tree = self.get_tree(column)?;
        let _ = tree.merge(key, value).map_err(Error::from)?;
        Ok(())
    }

    fn get(&self, column: &'static str, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let tree = self.get_tree(column)?;
        tree.get(key)
            .map(|value| value.map(|v| v.to_vec()))
            .map_err(Error::from)
    }

    fn contains(&self, column: &'static str, key: &[u8]) -> Result<bool, Error> {
        let tree = self.get_tree(column)?;
        tree.contains_key(key).map_err(Error::from)
    }

    fn write_batch(
        &self,
        column: &'static str,
        batch: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), Error> {
        let mut sled_batch = sled::Batch::default();
        let tree = self.get_tree(column)?;
        for (k, v) in batch {
            sled_batch.insert(k, v)
        }
        tree.apply_batch(sled_batch).map_err(Error::from)
    }

    fn flush(&self) -> Result<usize, Error> {
        self.db.flush().map_err(Error::from)
    }

    fn find(
        &self,
        column: &'static str,
        mode: BackendIteratorMode,
        limit: Option<usize>,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error> {
        let tree = self.get_tree(column)?;

        let iter = match mode {
            BackendIteratorMode::Start => SledDBIterator::new(SledDBIteratorMode::Start, tree),
            BackendIteratorMode::End => SledDBIterator::new(SledDBIteratorMode::End, tree),
            BackendIteratorMode::From(key, direction) => {
                SledDBIterator::new(SledDBIteratorMode::From(IVec::from(key), direction), tree)
            }
        };
        let mut results = Vec::new();
        if let Some(limit) = limit {
            let mut found = 0;
            for result in iter {
                let (key, value) = result.map_err(Error::from)?;
                if filter((key.as_ref(), value.as_ref()))? && found < limit {
                    results.push((
                        key.to_vec().into_boxed_slice(),
                        value.to_vec().into_boxed_slice(),
                    ));
                    found += 1;
                }
            }
        } else {
            for result in iter {
                let (key, value) = result.map_err(Error::from)?;
                if filter((key.as_ref(), value.as_ref()))? {
                    results.push((
                        key.to_vec().into_boxed_slice(),
                        value.to_vec().into_boxed_slice(),
                    ))
                }
            }
        }
        Ok(results)
    }

    fn find_by_prefix(
        &self,
        column: &'static str,
        key: &Vec<u8>,
        max_key_len: usize,
        filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>,
    ) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error> {
        let tree = self.get_tree(column)?;
        let prefix_key = key[..max_key_len].to_vec();
        let iter = SledDBIterator::new(SledDBIteratorMode::Prefix(IVec::from(prefix_key)), tree);
        let mut results = Vec::new();
        for result in iter {
            let (key, value) = result.map_err(Error::from)?;
            if filter((key.as_ref(), value.as_ref()))? {
                results.push((
                    key.to_vec().into_boxed_slice(),
                    value.to_vec().into_boxed_slice(),
                ))
            }
        }
        Ok(results)
    }
}

impl TezdegeDatabaseBackendKV for SledDBBackend {}

#[derive(Clone)]
pub enum SledDBIteratorMode {
    Start,
    End,
    From(IVec, Direction),
    Prefix(IVec),
}

pub struct SledDBIterator {
    mode: SledDBIteratorMode,
    iter: sled::Iter,
}

impl SledDBIterator {
    fn new(mode: SledDBIteratorMode, tree: Tree) -> Self {
        match mode.clone() {
            SledDBIteratorMode::Start => Self {
                mode,
                iter: tree.iter(),
            },
            SledDBIteratorMode::End => Self {
                mode,
                iter: tree.iter(),
            },
            SledDBIteratorMode::From(key, direction) => {
                let iter = match direction {
                    Direction::Forward => tree.range(key..),
                    Direction::Reverse => tree.range(..=key),
                };

                Self { mode, iter }
            }
            SledDBIteratorMode::Prefix(key) => Self {
                mode,
                iter: tree.scan_prefix(key),
            },
        }
    }
}

fn convert_next(
    item: Option<Result<(IVec, IVec), sled::Error>>,
) -> Option<Result<(Vec<u8>, Vec<u8>), Error>> {
    match item {
        None => None,
        Some(item) => match item {
            Ok((k, v)) => Some(Ok((k.to_vec(), v.to_vec()))),
            Err(error) => Some(Err(Error::SledDBError { error })),
        },
    }
}

impl Iterator for SledDBIterator {
    type Item = Result<(Vec<u8>, Vec<u8>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.mode {
            SledDBIteratorMode::Start => convert_next(self.iter.next()),
            SledDBIteratorMode::End => convert_next(self.iter.next_back()),
            SledDBIteratorMode::From(_, Direction::Forward) => convert_next(self.iter.next()),
            SledDBIteratorMode::From(_, Direction::Reverse) => convert_next(self.iter.next_back()),
            SledDBIteratorMode::Prefix(_) => convert_next(self.iter.next()),
        }
    }
}
