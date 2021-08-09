use std::path::{Path, PathBuf};
use crate::database::error::Error;
use crate::database::tezedge_database::{TezedgeDatabaseBackend, KVStoreKeyValueSchema, TezdegeDatabaseBackendKV};
use crate::database::backend::{TezedgeDatabaseBackendStore, BackendIteratorMode, DBStats, BackendIterator};
use crate::persistent::SchemaError;
use crate::{OperationsMetaStorage, BlockMetaStorage, Direction, operations_meta_storage, block_meta_storage};
use std::collections::HashMap;
use edgekv::edgekv::EdgeKV;
use std::sync::{RwLock, Arc, RwLockReadGuard};
use std::time::{Instant, Duration};
use std::collections::hash_map::RandomState;

pub struct EdgeKVBackend {
    column_stats : Arc<RwLock<HashMap<&'static str, DBStats>>>,
    db: HashMap<&'static str, EdgeKV>,
}

impl EdgeKVBackend {
    pub fn new<P: AsRef<Path>>(path: P, columns : Vec<&'static str>) -> Result<Self, Error> {
        let mut db = HashMap::with_capacity(columns.len());
        let mut p = PathBuf::new();
        p.push(path);
        for col in columns {
            let mut path = p.clone();
            path.push(col);
            let col_db = EdgeKV::open(path.as_path()).map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })?;
            db.insert(col, col_db);
        }

        Ok(Self { column_stats: Arc::new(Default::default()), db })
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
    iter: edgekv::edgekv::DBIterator,
}

impl NotusDBIterator {
    fn new(mode: NotusDBIteratorMode, db: &EdgeKV) -> Self {
        match mode.clone() {
            NotusDBIteratorMode::Start => Self {
                mode,
                iter: db.iter(),
            },
            NotusDBIteratorMode::End => Self {
                mode,
                iter: db.iter(),
            },
            NotusDBIteratorMode::From(key, direction) => {
                let iter = match direction {
                    Direction::Forward => db.range(key..),
                    Direction::Reverse => db.range(..=key),
                };

                Self { mode, iter }
            }
            NotusDBIteratorMode::Prefix(key) => Self {
                mode,
                iter: db.prefix(&key),
            },
        }
    }
}
impl TezdegeDatabaseBackendKV for EdgeKVBackend {}

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


impl TezedgeDatabaseBackendStore for EdgeKVBackend {
    fn put(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {

        let mut stats = self.column_stats.write().map_err(|e|{
            Error::GuardPoison { error: format!("{}", e) }
        })?;


        let timer = Instant::now();

        let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;

        db.put( key.to_vec(), value.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })?;

        let total_write_duration = timer.elapsed();
        let mut stat = stats.entry(column).or_insert(Default::default());
        stat.total_write_duration += total_write_duration;
        stat.total_writes += 1;

        stat.current_write_duration = total_write_duration;
        Ok(())
    }

    fn delete(&self, column: &'static str, key: &[u8]) -> Result<(), Error> {

        let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;

        db.delete(&key.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })?;
        Ok(())
    }

    fn merge(&self, column: &'static str, key: &[u8], value: &[u8]) -> Result<(), Error> {
        if column == OperationsMetaStorage::column_name() {
            let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;
            db.merge(
                    operations_meta_storage::merge_meta_value_notus,
                    key.to_vec(),
                    value.to_vec(),
                )
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        } else if column == BlockMetaStorage::column_name() {
            let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;
            db.merge(
                    block_meta_storage::merge_meta_value_notus,
                    key.to_vec(),
                    value.to_vec(),
                )
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        } else {
            let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;
            db
                .put(key.to_vec(), value.to_vec())
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        }
        Ok(())
    }

    fn get(&self, column: &'static str, key: &[u8]) -> Result<Option<Vec<u8>>, Error> {

        let mut stats = self.column_stats.write().map_err(|e|{
            Error::GuardPoison { error: format!("{}", e) }
        })?;

        let timer = Instant::now();

        let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;
        let value = db.get(&key.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })?;
        let total_read_duration = timer.elapsed();
        let mut stat = stats.entry(column).or_insert(Default::default());
        stat.total_read_duration += total_read_duration;
        stat.total_reads += 1;

        stat.current_read_duration = total_read_duration;
        Ok(value)
    }

    fn contains(&self, column: &'static str, key: &[u8]) -> Result<bool, Error> {
        let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;
        db.contains(&key.to_vec())
            .map_err(|error| Error::NutosError {
                error: format!("{:?}", error),
            })
    }

    fn write_batch(&self, column: &'static str, batch: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), Error> {
        let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;
        for (key, value) in batch {
            db.put(key, value)
                .map_err(|error| Error::NutosError {
                    error: format!("{:?}", error),
                })?;
        }
        Ok(())
    }

    fn flush(&self) -> Result<usize, Error> {
        //Todo: expose flush in database
        /*self.db.iter().for_each(|db|{

        });*/
        Ok(0)
    }

    fn size(&self) -> HashMap<&'static str, usize> {
        self.db.iter().map(|(column, db)| {
            (*column, db.size())
        }).collect()
    }

    fn sync(&self) -> Result<(), Error> {
        for (_,db) in  self.db.iter() {
            db.sync_all().map_err(|e| {
                Error::NutosError { error: format!("Notus Error: {:?}", e) }
            })?;
        }
        Ok(())
    }

    fn find<'a>(&'a self, column: &'static str, mode: BackendIteratorMode) -> Result<BackendIterator<'a>, Error> {
        let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;

        let iter = match mode {
            BackendIteratorMode::Start => {
                NotusDBIterator::new(NotusDBIteratorMode::Start, db)
            }
            BackendIteratorMode::End => {
                NotusDBIterator::new(NotusDBIteratorMode::End, db)
            }
            BackendIteratorMode::From(key, direction) => {
                NotusDBIterator::new(NotusDBIteratorMode::From(key, direction), db)
            }
        };

        return Ok(Box::new(iter.map(|result| {
            result.map(|(k, v)| (k.into_boxed_slice(), v.into_boxed_slice()))
        })))
    }

    fn find_by_prefix<'a>(&'a self, column: &'static str, key: &Vec<u8>, max_key_len: usize) -> Result<BackendIterator<'a>, Error> {
        let db = self.db.get(column).ok_or(Error::NutosError { error: format!("Column Missing: {}", column) })?;

        let prefix_key = key[..max_key_len].to_vec();
        let iter = NotusDBIterator::new(NotusDBIteratorMode::Prefix(prefix_key), db);

        return Ok(Box::new(iter.map(|result| {
            result.map(|(k, v)| (k.into_boxed_slice(), v.into_boxed_slice()))
        })))
    }

    fn column_stats(&self) -> HashMap<&'static str, DBStats> {
        let stats = match self.column_stats.read().map_err(|e|{
            Error::GuardPoison { error: format!("{}", e) }
        }) {
            Ok(stats) => {
                stats
            }
            Err(_) => {
                return Default::default()
            }
        };
        stats.clone()
    }
}