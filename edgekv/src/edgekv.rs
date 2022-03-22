// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![allow(clippy::ptr_arg)]

use crate::datastore::{DataStore, MergeOperator};

use crate::Result;

use std::fmt::{Display, Formatter};
use std::ops::RangeBounds;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

// TODO - TE-721: dir and config are not used
pub struct EdgeKV {
    store: Arc<DataStore>,
    dropped: Arc<AtomicBool>,
    config: EdgeKVConfiguration,
}

impl Display for EdgeKV {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut out = String::new();
        for (k, v) in self.iter().flatten() {
            out.push_str(&format!("{:?} : {:?} \n", k, v))
        }
        writeln!(f, "{}", out)
    }
}
#[derive(Copy, Clone)]
pub struct EdgeKVConfiguration {
    pub write_threshold: usize,
}

const DEFAULT_WRITE_THRESHOLD: usize = 1000;

impl Default for EdgeKVConfiguration {
    fn default() -> Self {
        Self {
            write_threshold: DEFAULT_WRITE_THRESHOLD,
        }
    }
}

impl EdgeKV {
    pub fn open<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let store = Arc::new(DataStore::open(dir.as_ref())?);
        let config = EdgeKVConfiguration::default();
        let instance = Self {
            store,
            dropped: Arc::new(AtomicBool::new(false)),
            config,
        };
        //Only for testing
        let mut p = PathBuf::new();
        p.push(dir);
        let worker_name = p
            .file_name()
            .map_or("".to_string(), |name| name.to_string_lossy().to_string());
        instance.start_background_workers(worker_name)?;
        Ok(instance)
    }

    pub fn open_with_configuration<P: AsRef<Path>>(
        dir: P,
        config: EdgeKVConfiguration,
    ) -> Result<Self> {
        let store = Arc::new(DataStore::open(dir.as_ref())?);
        let instance = Self {
            store,
            dropped: Arc::new(AtomicBool::new(false)),
            config,
        };
        //Only for testing
        let mut p = PathBuf::new();
        p.push(dir);
        let worker_name = p
            .file_name()
            .map_or("".to_string(), |name| name.to_string_lossy().to_string());
        instance.start_background_workers(worker_name)?;
        Ok(instance)
    }

    fn start_background_workers(&self, worker_name: String) -> Result<()> {
        let is_dropped = self.dropped.clone();
        let store = self.store.clone();
        let config = self.config;
        //let max_hint_file_size = self.config.max_hint_file_size;
        thread::Builder::new()
            .name(format!("edgekv-{}", worker_name))
            .spawn(move || {
                loop {
                    thread::sleep(Duration::from_millis(1));
                    let is_dropped = is_dropped.load(Ordering::Acquire);
                    if is_dropped {
                        break;
                    }
                    match store.sync_all(store.buffer_size() >= config.write_threshold) {
                        Ok(_) => {}
                        Err(e) => {
                            //Todo log Error
                            println!("Error {:?}", e)
                        }
                    }
                }
                drop(store)
            })?;
        Ok(())
    }
    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {
        self.store.put(key, value)?;
        Ok(())
    }
    pub fn get(&self, key: &Vec<u8>) -> Result<Option<Vec<u8>>> {
        if key.is_empty() {
            return Ok(None);
        }
        self.store.get(key)
    }

    pub fn contains(&self, key: &Vec<u8>) -> Result<bool> {
        if key.is_empty() {
            return Ok(false);
        }
        self.store.contains(key)
    }

    pub fn delete(&self, key: &Vec<u8>) -> Result<()> {
        if key.is_empty() {
            return Ok(());
        }
        self.store.delete(key)
    }

    pub fn compact(&self) -> Result<()> {
        self.store.merge()
    }

    pub fn clear(&self) -> Result<()> {
        self.store.clear()
    }

    pub fn merge(
        &self,
        merge_operator: impl MergeOperator + 'static,
        key: Vec<u8>,
        value: Vec<u8>,
    ) -> Result<()> {
        let old_value = self.store.get(&key)?;
        let merged_value = merge_operator(&key, old_value, &value);
        match merged_value {
            None => {
                self.delete(&key)?;
            }
            Some(value) => {
                self.put(key, value)?;
            }
        }
        Ok(())
    }
    pub fn iter(&self) -> DBIterator {
        DBIterator::new(self.store.clone())
    }

    pub fn range<R>(&self, range: R) -> DBIterator
    where
        R: RangeBounds<Vec<u8>>,
    {
        DBIterator::range(self.store.clone(), range)
    }

    pub fn prefix(&self, prefix: &Vec<u8>) -> DBIterator {
        DBIterator::prefix(self.store.clone(), prefix)
    }

    pub fn size(&self) -> usize {
        self.store.size()
    }

    pub fn sync_all(&self) -> Result<()> {
        self.store.sync_all(true)
    }
}

impl Drop for EdgeKV {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
        // TODO - TE-721: handle this error
        let _ = self.store.sync_all(false);
    }
}

pub struct DBIterator {
    store: Arc<DataStore>,
    inner: Vec<Arc<Vec<u8>>>,
    cursor: usize,
}

impl DBIterator {
    pub(crate) fn new(store: Arc<DataStore>) -> Self {
        let keys = store.keys();
        Self {
            store,
            inner: keys,
            cursor: 0,
        }
    }

    fn range<R>(store: Arc<DataStore>, range: R) -> Self
    where
        R: RangeBounds<Vec<u8>>,
    {
        let keys = store.range(range);
        Self {
            store,
            inner: keys,
            cursor: 0,
        }
    }

    fn prefix(store: Arc<DataStore>, prefix: &Vec<u8>) -> Self {
        let keys = store.prefix(prefix);
        Self {
            store,
            inner: keys,
            cursor: 0,
        }
    }
}

impl Iterator for DBIterator {
    type Item = Result<(Vec<u8>, Vec<u8>)>;

    fn next(&mut self) -> Option<Self::Item> {
        let key = match self.inner.get(self.cursor) {
            None => {
                return None;
            }
            Some(key) => key,
        };
        match self.store.get(key) {
            Ok(Some(value)) => {
                self.cursor += 1;
                Some(Ok((key.to_vec(), value)))
            }
            _ => None,
        }
    }
}

impl DoubleEndedIterator for DBIterator {
    fn next_back(&mut self) -> Option<Self::Item> {
        let position = match self.inner.len().checked_sub(1) {
            None => {
                return None;
            }
            Some(position) => match position.checked_sub(self.cursor) {
                None => {
                    return None;
                }
                Some(position) => position,
            },
        };

        let key = match self.inner.get(position) {
            None => {
                return None;
            }
            Some(key) => key,
        };

        match self.store.get(key) {
            Ok(Some(value)) => {
                self.cursor += 1;
                Some(Ok((key.to_vec(), value)))
            }
            _ => None,
        }
    }
}
