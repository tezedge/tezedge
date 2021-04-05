use crate::database::backend::{
    BackendIterator, BackendIteratorMode, TezedgeDatabaseBackendStore,
    TezedgeDatabaseBackendStoreIterator,
};
use crate::database::error::Error;
use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezdegeDatabaseBackendKV};
use crate::{
    block_meta_storage, operations_meta_storage, BlockMetaStorage, Direction, OperationsMetaStorage,
};
use notus::nutos::Notus;
use std::path::Path;

#[derive(Clone)]
pub enum NotusDBIteratorMode {
    Start,
    End,
    From(Vec<u8>, Direction),
    Prefix(Vec<u8>),
}

pub struct NotusDBIterator {
    mode: NotusDBIteratorMode,
    iter: notus::nutos::DBIterator,
}

impl NotusDBIterator {
    fn new(mode: NotusDBIteratorMode, db: &Notus, column: &str) -> Self {
        match mode.clone() {
            NotusDBIteratorMode::Start => Self {
                mode,
                iter: db.iter_cf(column),
            },
            NotusDBIteratorMode::End => Self {
                mode,
                iter: db.iter_cf(column),
            },
            NotusDBIteratorMode::From(key, direction) => {
                let iter = match direction {
                    Direction::Forward => db.range_cf(column, key..),
                    Direction::Reverse => db.range_cf(column, ..=key),
                };

                Self { mode, iter }
            }
            NotusDBIteratorMode::Prefix(key) => Self {
                mode,
                iter: db.prefix_cf(column, &key),
            },
        }
    }
}
impl TezdegeDatabaseBackendKV for NotusDBBackend {}

impl Iterator for NotusDBIterator {
    type Item = Result<(Vec<u8>, Vec<u8>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match &self.mode {
            NotusDBIteratorMode::Start => self.iter.next(),
            NotusDBIteratorMode::End => self.iter.next_back(),
            NotusDBIteratorMode::From(_, direction) => match direction {
                Direction::Forward => self.iter.next(),
                Direction::Reverse => self.iter.next_back(),
            },
            NotusDBIteratorMode::Prefix(_) => self.iter.next(),
        };
        match next {
            None => None,
            Some(item) => match item {
                Ok((k, v)) => Some(Ok((k.to_vec(), v.to_vec()))),
                Err(error) => Some(Err(Error::NutosError {
                    error: format!("{}", error),
                })),
            },
        }
    }
}

pub struct NotusDBBackend {
    db: Notus,
}

impl NotusDBBackend {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let db = Notus::open(path).map_err(|error| Error::NutosError {
            error: format!("{}", error),
        })?;
        Ok(Self { db })
    }
}

impl TezedgeDatabaseBackendStore for NotusDBBackend {
    fn put(&self, column: &'static str, key: Vec<u8>, value: Vec<u8>) -> Result<(), Error> {
        self.db
            .put_cf(column, key, value)
            .map_err(|error| Error::NutosError {
                error: format!("{}", error),
            })?;
        Ok(())
    }

    fn delete(&self, column: &'static str, key: Vec<u8>) -> Result<(), Error> {
        self.db
            .delete_cf(column, &key)
            .map_err(|error| Error::NutosError {
                error: format!("{}", error),
            })?;
        Ok(())
    }

    fn merge(&self, column: &'static str, key: Vec<u8>, value: Vec<u8>) -> Result<(), Error> {
        if column == OperationsMetaStorage::column_name() {
            self.db
                .merge_cf(
                    operations_meta_storage::merge_meta_value_notus,
                    column,
                    key,
                    value,
                )
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        } else if column == BlockMetaStorage::column_name() {
            self.db
                .merge_cf(
                    block_meta_storage::merge_meta_value_notus,
                    column,
                    key,
                    value,
                )
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        } else {
            self.db
                .put_cf(column, key, value)
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        }
        Ok(())
    }

    fn get(&self, column: &'static str, key: Vec<u8>) -> Result<Option<Vec<u8>>, Error> {
        self.db
            .get_cf(column, &key)
            .map_err(|error| Error::NutosError {
                error: format!("{}", error),
            })
    }

    fn contains(&self, column: &'static str, key: Vec<u8>) -> Result<bool, Error> {
        self.db
            .contains_cf(column, &key)
            .map_err(|error| Error::NutosError {
                error: format!("{}", error),
            })
    }

    fn write_batch(
        &self,
        column: &'static str,
        batch: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), Error> {
        for (key, value) in batch {
            self.db
                .put_cf(column, key, value)
                .map_err(|error| Error::NutosError {
                    error: format!("{}", error),
                })?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<usize, Error> {
        Ok(0)
    }
}

impl TezedgeDatabaseBackendStoreIterator for NotusDBBackend {
    fn iterator(
        &self,
        column: &'static str,
        mode: BackendIteratorMode,
    ) -> Result<Box<BackendIterator>, Error> {
        let iter = match mode {
            BackendIteratorMode::Start => {
                NotusDBIterator::new(NotusDBIteratorMode::Start, &self.db, column)
            }
            BackendIteratorMode::End => {
                NotusDBIterator::new(NotusDBIteratorMode::End, &self.db, column)
            }
            BackendIteratorMode::From(key, direction) => {
                NotusDBIterator::new(NotusDBIteratorMode::From(key, direction), &self.db, column)
            }
        };
        Ok(Box::new(iter))
    }

    fn prefix_iterator(
        &self,
        column: &'static str,
        key: &Vec<u8>,
        max_key_len: usize,
    ) -> Result<Box<BackendIterator>, Error> {
        let prefix_key = key[..max_key_len].to_vec();
        let iter = NotusDBIterator::new(NotusDBIteratorMode::Prefix(prefix_key), &self.db, column);
        Ok(Box::new(iter))
    }
}
