use notus::nutos::Notus;
use std::path::Path;
use crate::database::error::Error;
use crate::database::tezedge_database::{TezedgeDatabaseBackend, KVStoreKeyValueSchema, TezdegeDatabaseBackendKV};
use crate::database::backend::{TezedgeDatabaseBackendStore, BackendIteratorMode};
use crate::persistent::SchemaError;
use crate::{OperationsMetaStorage, BlockMetaStorage, Direction, operations_meta_storage, block_meta_storage};

pub struct NotusDBBackend {
    db: Notus,
}

impl NotusDBBackend {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        let db = Notus::open(path).map_err(|error| Error::NutosError {
            error: format!("{:?}", error),
        })?;
        Ok(Self { db })
    }
}

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
                    error: format!("{:?}", error),
                })),
            },
        }
    }
}


impl TezedgeDatabaseBackendStore for NotusDBBackend {
    fn put(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        self.db
            .put_cf(column, key.to_vec(), value.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })?;
        Ok(())
    }

    fn delete(&self, column: &'static str, key: &[u8]) -> Result<(), Error> {
        self.db
            .delete_cf(column, &key.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })?;
        Ok(())
    }

    fn merge(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        if column == OperationsMetaStorage::column_name() {
            self.db
                .merge_cf(
                    operations_meta_storage::merge_meta_value_notus,
                    column,
                    key.to_vec(),
                    value.to_vec(),
                )
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        } else if column == BlockMetaStorage::column_name() {
            self.db
                .merge_cf(
                    block_meta_storage::merge_meta_value_notus,
                    column,
                    key.to_vec(),
                    value.to_vec(),
                )
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        } else {
            self.db
                .put_cf(column, key.to_vec(), value.to_vec())
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        }
        Ok(())
    }

    fn get(&self, column: &'static str, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        self.db
            .get_cf(column, &key.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })
    }

    fn contains(&self, column: &'static str, key: &[u8]) -> Result<bool, Error> {
        self.db
            .contains_cf(column, &key.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })
    }

    fn write_batch(&self, column: &'static str, batch: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), Error> {
        for (key, value) in batch {
            self.db
                .put_cf(column, key, value)
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<usize, Error> {
        Ok(0)
    }

    fn find(&self, column: &'static str, mode: BackendIteratorMode, limit: Option<usize>, filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error> {
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

    fn find_by_prefix(&self, column: &'static str, key: &Vec<u8>, max_key_len: usize, filter: Box<dyn Fn((&[u8], &[u8])) -> Result<bool, SchemaError>>) -> Result<Vec<(Box<[u8]>, Box<[u8]>)>, Error> {
        let prefix_key = key[..max_key_len].to_vec();
        let iter = NotusDBIterator::new(NotusDBIteratorMode::Prefix(prefix_key), &self.db, column);
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