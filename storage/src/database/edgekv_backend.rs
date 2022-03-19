// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::database::backend::{
    BackendIterator, BackendIteratorMode, DBStats, TezedgeDatabaseBackendStore,
};
use crate::database::error::Error;
use crate::database::tezedge_database::{KVStoreKeyValueSchema, TezdegeDatabaseBackendKV};
use crate::{
    block_meta_storage, operations_meta_storage, BlockMetaStorage, Direction, OperationsMetaStorage,
};
use edgekv::edgekv::EdgeKV;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Instant;

pub struct EdgeKVBackend {
    column_stats: Arc<RwLock<HashMap<&'static str, DBStats>>>,
    db: HashMap<&'static str, EdgeKV>,
}

impl EdgeKVBackend {
    pub fn new<P: AsRef<Path>>(path: P, columns: Vec<&'static str>) -> Result<Self, Error> {
        let mut db = HashMap::with_capacity(columns.len());
        let mut p = PathBuf::new();
        p.push(path);
        for col in columns {
            let mut path = p.clone();
            path.push(col);
            let col_db = EdgeKV::open(path.as_path()).map_err(|error| Error::EdgeKVError {
                error: format!("{:?}", error),
            })?;
            db.insert(col, col_db);
        }

        Ok(Self {
            column_stats: Arc::new(Default::default()),
            db,
        })
    }
}

#[derive(Clone)]
pub enum EdgeKVIteratorMode {
    Start,
    End,
    From(Vec<u8>, Direction),
    Prefix(Vec<u8>),
}

pub struct EdgeKVIterator {
    mode: EdgeKVIteratorMode,
    iter: edgekv::edgekv::DBIterator,
}

impl EdgeKVIterator {
    fn new(mode: EdgeKVIteratorMode, db: &EdgeKV) -> Self {
        match mode.clone() {
            EdgeKVIteratorMode::Start => Self {
                mode,
                iter: db.iter(),
            },
            EdgeKVIteratorMode::End => Self {
                mode,
                iter: db.iter(),
            },
            EdgeKVIteratorMode::From(key, direction) => {
                let iter = match direction {
                    Direction::Forward => db.range(key..),
                    Direction::Reverse => db.range(..=key),
                };

                Self { mode, iter }
            }
            EdgeKVIteratorMode::Prefix(key) => Self {
                mode,
                iter: db.prefix(&key),
            },
        }
    }
}
impl TezdegeDatabaseBackendKV for EdgeKVBackend {}

impl Iterator for EdgeKVIterator {
    type Item = Result<(Vec<u8>, Vec<u8>), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = match &self.mode {
            EdgeKVIteratorMode::Start => self.iter.next(),
            EdgeKVIteratorMode::End => self.iter.next_back(),
            EdgeKVIteratorMode::From(_, direction) => match direction {
                Direction::Forward => self.iter.next(),
                Direction::Reverse => self.iter.next_back(),
            },
            EdgeKVIteratorMode::Prefix(_) => self.iter.next(),
        };
        match next {
            None => None,
            Some(item) => match item {
                Ok((k, v)) => Some(Ok((k.to_vec(), v.to_vec()))),
                Err(error) => Some(Err(Error::EdgeKVError {
                    error: format!("{:?}", error),
                })),
            },
        }
    }
}

impl TezedgeDatabaseBackendStore for EdgeKVBackend {
    fn put(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let mut stats = self.column_stats.write().map_err(|e| Error::GuardPoison {
            error: format!("{}", e),
        })?;

        let timer = Instant::now();

        let db = self.db.get(column).ok_or(Error::EdgeKVError {
            error: format!("Column Missing: {}", column),
        })?;

        db.put(key.to_vec(), value.to_vec())
            .map_err(|error| Error::EdgeKVError {
                error: format!("{:?}", error),
            })?;

        let total_write_duration = timer.elapsed();
        let mut stat = stats.entry(column).or_insert_with(Default::default);
        stat.total_write_duration += total_write_duration;
        stat.total_writes += 1;

        Ok(())
    }

    fn delete(&self, column: &'static str, key: &[u8]) -> Result<(), Error> {
        let db = self.db.get(column).ok_or(Error::EdgeKVError {
            error: format!("Column Missing: {}", column),
        })?;

        db.delete(&key.to_vec())
            .map_err(|error| Error::EdgeKVError {
                error: format!("{:?}", error),
            })?;
        Ok(())
    }

    fn merge(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        let mut stats = self.column_stats.write().map_err(|e| Error::GuardPoison {
            error: format!("{}", e),
        })?;

        let timer = Instant::now();

        if column == OperationsMetaStorage::column_name() {
            let db = self.db.get(column).ok_or(Error::EdgeKVError {
                error: format!("Column Missing: {}", column),
            })?;
            db.merge(
                operations_meta_storage::merge_meta_value_edgekv,
                key.to_vec(),
                value.to_vec(),
            )
            .map_err(|error| Error::EdgeKVError {
                error: format!("{:?}", error),
            })?;
        } else if column == BlockMetaStorage::column_name() {
            let db = self.db.get(column).ok_or(Error::EdgeKVError {
                error: format!("Column Missing: {}", column),
            })?;
            db.merge(
                block_meta_storage::merge_meta_value_edgekv,
                key.to_vec(),
                value.to_vec(),
            )
            .map_err(|error| Error::EdgeKVError {
                error: format!("{:?}", error),
            })?;
        } else {
            let db = self.db.get(column).ok_or(Error::EdgeKVError {
                error: format!("Column Missing: {}", column),
            })?;
            db.put(key.to_vec(), value.to_vec())
                .map_err(|error| Error::EdgeKVError {
                    error: format!("{:?}", error),
                })?;
        }

        let total_update_duration = timer.elapsed();
        let mut stat = stats.entry(column).or_insert_with(Default::default);
        stat.total_update_duration += total_update_duration;
        stat.total_updates += 1;
        Ok(())
    }

    fn get(&self, column: &'static str, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        let mut stats = self.column_stats.write().map_err(|e| Error::GuardPoison {
            error: format!("{}", e),
        })?;

        let timer = Instant::now();

        let db = self.db.get(column).ok_or(Error::EdgeKVError {
            error: format!("Column Missing: {}", column),
        })?;
        let value = db.get(&key.to_vec()).map_err(|error| Error::EdgeKVError {
            error: format!("{:?}", error),
        })?;
        let total_read_duration = timer.elapsed();
        let mut stat = stats.entry(column).or_insert_with(Default::default);
        stat.total_read_duration += total_read_duration;
        stat.total_reads += 1;

        Ok(value)
    }

    fn contains(&self, column: &'static str, key: &[u8]) -> Result<bool, Error> {
        let db = self.db.get(column).ok_or(Error::EdgeKVError {
            error: format!("Column Missing: {}", column),
        })?;
        db.contains(&key.to_vec())
            .map_err(|error| Error::EdgeKVError {
                error: format!("{:?}", error),
            })
    }

    fn write_batch(
        &self,
        column: &'static str,
        batch: Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), Error> {
        let db = self.db.get(column).ok_or(Error::EdgeKVError {
            error: format!("Column Missing: {}", column),
        })?;
        for (key, value) in batch {
            db.put(key, value).map_err(|error| Error::EdgeKVError {
                error: format!("{:?}", error),
            })?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<usize, Error> {
        for (_, db) in self.db.iter() {
            db.sync_all().map_err(|e| Error::EdgeKVError {
                error: format!("EdgeKV Error: {:?}", e),
            })?;
        }
        Ok(0)
    }

    fn size(&self) -> HashMap<&'static str, usize> {
        self.db
            .iter()
            .map(|(column, db)| (*column, db.size()))
            .collect()
    }

    fn sync(&self) -> Result<(), Error> {
        for (_, db) in self.db.iter() {
            db.sync_all().map_err(|e| Error::EdgeKVError {
                error: format!("EdgeKV Error: {:?}", e),
            })?;
        }
        Ok(())
    }

    fn find<'a>(
        &'a self,
        column: &'static str,
        mode: BackendIteratorMode,
    ) -> Result<BackendIterator<'a>, Error> {
        let db = self.db.get(column).ok_or(Error::EdgeKVError {
            error: format!("Column Missing: {}", column),
        })?;

        let iter = match mode {
            BackendIteratorMode::Start => EdgeKVIterator::new(EdgeKVIteratorMode::Start, db),
            BackendIteratorMode::End => EdgeKVIterator::new(EdgeKVIteratorMode::End, db),
            BackendIteratorMode::From(key, direction) => {
                EdgeKVIterator::new(EdgeKVIteratorMode::From(key, direction), db)
            }
        };

        Ok(Box::new(iter.map(|result| {
            result.map(|(k, v)| (k.into_boxed_slice(), v.into_boxed_slice()))
        })))
    }

    fn find_by_prefix<'a>(
        &'a self,
        column: &'static str,
        key: &Vec<u8>,
        max_key_len: usize,
    ) -> Result<BackendIterator<'a>, Error> {
        let db = self.db.get(column).ok_or(Error::EdgeKVError {
            error: format!("Column Missing: {}", column),
        })?;

        let prefix_key = key[..max_key_len].to_vec();
        let iter = EdgeKVIterator::new(EdgeKVIteratorMode::Prefix(prefix_key), db);

        Ok(Box::new(iter.map(|result| {
            result.map(|(k, v)| (k.into_boxed_slice(), v.into_boxed_slice()))
        })))
    }

    fn column_stats(&self) -> HashMap<&'static str, DBStats> {
        let stats = match self.column_stats.read().map_err(|e| Error::GuardPoison {
            error: format!("{}", e),
        }) {
            Ok(stats) => stats,
            Err(_) => return Default::default(),
        };
        stats.clone()
    }
}
