use crate::datastore::DataIndex::Persisted;
use crate::errors::EdgeKVError;
use crate::file_ops::{create_new_file_pair, fetch_file_pairs, get_lock_file, ActiveFilePair, FilePair, Index};
use crate::schema::{DataEntry, Encoder, Decoder, HintEntry};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::alloc::Global;
use std::collections::{BTreeMap, HashMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::ops::{RangeFrom, RangeBounds, Range, RangeInclusive, RangeToInclusive, RangeFull, Bound, Mul, Div, Add, DerefMut, Deref};
use std::path::{Path, PathBuf};
use std::sync::{RwLock, Arc, PoisonError, RwLockReadGuard, RwLockWriteGuard};
use std::ops;

use crate::Result;
use std::io::{Read, Cursor, Write, BufReader};
use std::sync::atomic::{AtomicI64, Ordering};
use std::cmp::{min, max};
use chrono::{Date, Utc};
use std::option::Option::Some;
use slab::Slab;
use lru_cache::LruCache;
use bloomfilter::Bloom;
use std::rc::Rc;
use std::collections::hash_map::RandomState;
use std::collections::btree_map::Iter;
use std::time::Instant;
use crate::fifo::FifoCache;
use crate::ivec::IVec;

pub trait MergeOperator: Fn(&[u8], Option<Vec<u8>>, &[u8]) -> Option<Vec<u8>> {}

impl<F> MergeOperator for F where F: Fn(&[u8], Option<Vec<u8>>, &[u8]) -> Option<Vec<u8>> {}

pub struct Column {
    merge_operator: Box<dyn MergeOperator>,
}

#[derive(Default, Debug, Clone)]
pub struct KeyDirEntry {
    file_id: String,
    key_size: u64,
    value_size: u64,
    data_entry_position: u64,
}

#[derive(Debug, Clone)]
enum DataIndex {
    Persisted(KeyDirEntry),
    InBuffer,
}

impl DataIndex {
    pub fn size_in_mem(&self) -> usize {
        match self {
            Persisted(s) => {
                return s.size();
            }
            DataIndex::InBuffer => {}
        }
        std::mem::size_of_val(self)
    }
}

impl KeyDirEntry {
    pub fn new(file_id: String, key_size: u64, value_size: u64, pos: u64) -> Self {
        KeyDirEntry {
            file_id,
            key_size,
            value_size,
            data_entry_position: pos,
        }
    }

    pub fn size(&self) -> usize {
        std::mem::size_of_val(&self.key_size)
            + std::mem::size_of_val(&self.value_size)
            + std::mem::size_of_val(&self.data_entry_position)
            + self.file_id.len()
    }
}

type MultiMap<I, K, V> = BTreeMap<I, BTreeMap<K, V>>;

pub struct KeysDir {
    bloom_filters : RwLock<BTreeMap<String,Bloom<Vec<u8>>>>,
    keys : RwLock<BTreeMap<Vec<u8>, DataIndex>>
}

impl KeysDir {
    pub fn insert(&self, key: Vec<u8>, value: KeyDirEntry) -> Result<()> {
        let mut keys_dir_writer = self
            .keys
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        let index = DataIndex::Persisted(value);
        keys_dir_writer.insert(key, index);
        Ok(())
    }

    pub fn insert_bulk(&self, bulk: BTreeMap<Vec<u8>, KeyDirEntry>) -> Result<()> {
        let mut keys_dir_writer = self
            .keys
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        keys_dir_writer.extend(bulk.iter().map(|(k,v)| (k.clone(), DataIndex::Persisted(v.clone()))));
        Ok(())
    }

    pub fn insert_bloom(&self, file_id: String, value: Bloom<Vec<u8>>) -> Result<()> {
        let mut bloom_filters_writer = self
            .bloom_filters
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        bloom_filters_writer.insert(file_id, value);
        Ok(())
    }

    pub fn partial_insert(&self, key: Vec<u8>) -> Result<()> {
        let mut keys_dir_writer = self
            .keys
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        let index = DataIndex::InBuffer;
        keys_dir_writer.insert(key, index);
        Ok(())
    }
    pub fn remove(&self, key: &[u8]) -> Result<()> {
        let mut keys_dir_writer = self
            .keys
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        keys_dir_writer.remove(key);
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        let mut keys_dir_writer = self
            .keys
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        keys_dir_writer.clear();
        Ok(())
    }


    //TODO change to use references
    pub fn keys(&self) -> Vec<Vec<u8>> {
        let keys_dir_reader = match self.keys.read() {
            Ok(rdr) => rdr,
            Err(_) => {
                return vec![];
            }
        };

        keys_dir_reader.iter().map(|(k, _)| k.clone()).collect()
    }

    //TODO change to use references
    pub fn range<R>(&self, range : R) -> Vec<Vec<u8>> where R : RangeBounds<Vec<u8>> {
        let keys_dir_reader = match self.keys.read() {
            Ok(rdr) => rdr,
            Err(_) => {
                return vec![];
            }
        };
        keys_dir_reader.range(range).map(|(k, _)| k.clone()).collect()
    }

    //TODO change to use references
    pub fn prefix(&self, prefix: &Vec<u8>) -> Vec<Vec<u8>> {
        let keys_dir_reader = match self.keys.read() {
            Ok(rdr) => rdr,
            Err(_) => {
                return vec![];
            }
        };
        keys_dir_reader
            .range(prefix.clone()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.clone())
            .collect()
    }


    pub fn get(&self, key: &[u8]) -> Option<KeyDirEntry> {
        let keys_dir_reader = match self.keys.read() {
            Ok(rdr) => rdr,
            Err(_) => {
                return None;
            }
        };
        match keys_dir_reader.get(key) {
            None => None,
            Some(entry) => {
                if let Persisted(entry) = entry {
                    return Some(entry.clone());
                }
                return None;
            }
        }
    }

    pub fn size(&self) -> usize {
        let keys_dir_reader = match self.keys.read() {
            Ok(rdr) => rdr,
            Err(_) => {
                return 0;
            }
        };

        keys_dir_reader.len()
    }

    pub fn contains(&self, key: &[u8]) -> Result<bool> {

        let mut bloom_filters_writer = self
            .bloom_filters
            .read()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        for (_, bloomfilter) in bloom_filters_writer.iter() {
            if bloomfilter.check(&key.to_vec()) {
                return Ok(true)
            }
        }

        Ok(false)
    }
}

impl KeysDir {
    pub fn new(file_pairs: &BTreeMap<String, FilePair>) -> Result<Self> {
        let keys_dir = Self {
            bloom_filters: Default::default(),
            keys: Default::default()
        };
        for (_, fp) in file_pairs {
            fp.fetch_bloom_filters(&keys_dir)?;
            fp.fetch_hint_entries(&keys_dir)?;
        }
        Ok(keys_dir)
    }
}

pub struct IndexDir {
    indexes: RwLock<BTreeMap<String, Index>>,
}

impl IndexDir {
    pub fn new( file_pairs : BTreeMap<String,FilePair>) -> Result<IndexDir>{
        let mut indexes = BTreeMap::new();
        for (k,v) in file_pairs {
            match v.to_index() {
                Ok(index) => {
                    indexes.insert(k,index);
                }
                Err(_) => {}
            }

        }

        Ok(IndexDir {
            indexes: RwLock::new(indexes)
        })
    }

    pub fn indexes(&self) -> Result<RwLockReadGuard<BTreeMap<String,Index>>> {
        let indexes = self.indexes.read().map_err(|e|
            EdgeKVError::RWLockPoisonError(format!("{}", e))
        )?;

        Ok(indexes)
    }

    pub fn insert(&self, file_pair : FilePair) -> Result<()>{
        let mut indexes = self.indexes.write().map_err(|e|
            EdgeKVError::RWLockPoisonError(format!("{}", e))
        )?;
        indexes.insert(file_pair.file_id(), file_pair.to_index()?);
        Ok(())
    }
}

pub struct DataStore {
    lock_file: File,
    dir: PathBuf,
    active_file: RwLock<ActiveFilePair>,
    keys_dir: KeysDir,
    index_dir : IndexDir,
    buffer: RwLock<HashMap<Vec<u8>, Vec<u8>>>,
    double_buffer: HashMap<Vec<u8>, DataEntry>,
    buffer_size: RwLock<usize>
}

pub fn fetch_double_buffer_file(buffer_dir: &BTreeMap<i64, PathBuf>) -> Result<HashMap<Vec<u8>, DataEntry>> {
    let mut double_buffer = HashMap::new();
    for (_, file_path) in buffer_dir.iter() {
        let double_buffer_file = File::open(file_path)?;
        let mut rdr = BufReader::new(double_buffer_file);
        while let Ok(data_entry) = DataEntry::decode(&mut rdr) {
            if !data_entry.check_crc() {
                return Err(EdgeKVError::CorruptData);
            }
            double_buffer.insert(data_entry.key(), data_entry);
        }
    }
    Ok(double_buffer)
}

impl DataStore {
    pub fn open<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let lock_file = get_lock_file(dir.as_ref())?;
        let active_file_pair = create_new_file_pair(dir.as_ref())?;
        let (files_dir, buffer_files) = fetch_file_pairs(dir.as_ref())?;
        let double_buffer = fetch_double_buffer_file(&buffer_files)?;
        let keys_dir = KeysDir::new(&files_dir)?;
        let index_dir = IndexDir::new(files_dir)?;
        let mut instance = Self {
            lock_file,
            dir: dir.as_ref().to_path_buf(),
            active_file: RwLock::new(ActiveFilePair::from(active_file_pair)?),
            keys_dir,
            index_dir,
            buffer: RwLock::new(Default::default()),
            double_buffer,
            buffer_size: RwLock::new(0)
        };
        instance.lock()?;
        Ok(instance)
    }

    fn lock(&mut self) -> Result<()> {
        self.lock_file
            .lock_exclusive()
            .map_err(|_| EdgeKVError::LockFailed(String::from(self.dir.to_string_lossy())))?;
        Ok(())
    }

    pub fn keys_dir(&self) -> &KeysDir {
        &self.keys_dir
    }

    pub fn buffer_size(&self) -> usize {
        let mut buffer = match self
            .buffer
            .read() {
            Ok(buff) => {
                buff
            }
            Err(_) => {
                return 0;
            }
        };
        buffer.len()
    }

    pub fn put(&self, key: Vec<u8>, value: Vec<u8>) -> Result<()> {

        let mut buffer_size = self
            .buffer_size
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        *buffer_size = buffer_size.add(value.len() + key.len());

        let mut buffer = self
            .buffer
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        buffer.insert(key.clone(), value.clone());
        self.keys_dir.partial_insert(key);
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {

        let buffer = self
            .buffer
            .read()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        if let Some(value) = buffer.get(key) {
            return Ok(Some(value.clone()));
        }

        let key_dir_entry = if let Some(entry) = self.keys_dir.get(key) { entry }
        else {
            if let Some(data_entry) = self.double_buffer.get(key) {
                return Ok(Some(data_entry.value()))
            }
            return Ok(None)
        };


        let indexes_read_lock = self.index_dir.indexes()?;
        let index = indexes_read_lock.get(&key_dir_entry.file_id).ok_or(EdgeKVError::CorruptData)?;
        if let Ok(data_entry) = index.read(key_dir_entry.data_entry_position, key_dir_entry.size()) {
            return Ok(Some(data_entry.value()))
        }
        Ok(None)
    }

    pub fn delete(&self, key: &[u8]) -> Result<()> {
        let mut buffer = self
            .buffer
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        let active_file = self
            .active_file
            .read()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        buffer.remove(key);
        active_file.remove(key.to_vec())?;
        self.keys_dir.remove(key);
        Ok(())
    }

    pub fn contains(&self, key: &[u8]) -> Result<bool> {
        let mut buffer = self
            .buffer
            .read()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        if buffer.contains_key(key) {
            return Ok(true);
        }

        let result = self.keys_dir.contains(key)?;
        Ok(result)
    }

    pub fn clear(&self) -> Result<()> {
        let active_file = self
            .active_file
            .read()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        for key in self.keys().iter() {
            active_file.remove(key.to_vec())?;
        }
        self.keys_dir.clear()?;
        let mut buffer = self
            .buffer
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        buffer.clear();
        Ok(())
    }

    pub fn keys(&self) -> Vec<Vec<u8>> {
        self.keys_dir.keys()
    }

    pub fn try_split(&self, active_file: &mut ActiveFilePair) -> Result<()> {
        let active_file_pair = create_new_file_pair(self.dir.as_path())?;
        *active_file = ActiveFilePair::from(active_file_pair)?;
        Ok(())
    }


    pub fn range<R>(&self, range: R) -> Vec<Vec<u8>> where R: RangeBounds<Vec<u8>> {
        self.keys_dir.range(range)
    }

    pub fn active_file_hint_size(&self) -> usize {
        let active_file = match self.active_file.read() {
            Ok(active_file ) => {
                active_file
            }
            Err(_) => {
                return 0;
            }
        };
        match active_file.hint_file_size() {
            Ok(size) => {
                size as usize
            }
            Err(_) => {
                0
            }
        }
    }

    pub fn active_data_size(&self) -> usize {
        let active_file = match self.active_file.read() {
            Ok(active_file) => {
                active_file
            }
            Err(_) => {
                return 0;
            }
        };
        match active_file.data_file_size() {
            Ok(size) => {
                size as usize
            }
            Err(_) => {
                0
            }
        }
    }

    pub fn prefix(&self, prefix: &Vec<u8>) -> Vec<Vec<u8>> {
        self.keys_dir.prefix(prefix)
    }

    pub fn merge(&self) -> Result<()> {
        let mut active_file = self
            .active_file
            .read()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;
        let merged_file_pair = ActiveFilePair::from(create_new_file_pair(self.dir.as_path())?)?;
        let mut mark_for_removal  = Vec::new();

        for (_, index) in self.index_dir.indexes()?.iter() {
            if index.file_id() == active_file.file_id() {
                continue;
            }
            let hints = index.get_hints()?;
            for hint in hints {
                if let Some(keys_dir_entry) = self.keys_dir.get(&hint.key()) {
                    if keys_dir_entry.file_id == index.file_id() {
                        let data_entry = index.read(hint.data_entry_position(), hint.size())?;
                        let key_entry = merged_file_pair.write(&data_entry, &self.keys_dir)?;
                        self.keys_dir.insert(hint.key(), key_entry);
                    }
                }
            }
            mark_for_removal.push(index.data_file_path());
            mark_for_removal.push(index.hint_file_path());
        }

        fs_extra::remove_items(&mark_for_removal);
        Ok(())
    }

    pub fn sync_all(&self, split_active_file: bool) -> Result<()> {
        let mut active_file = self
            .active_file
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        let mut buffer = self
            .buffer
            .write()
            .map_err(|e| EdgeKVError::RWLockPoisonError(format!("{}", e)))?;

        let file_name = active_file.file_id();
        let mut buffer_file_path = PathBuf::new();
        buffer_file_path.push(self.dir.as_path());
        buffer_file_path.push(format!("{}.buff", file_name.as_str()));


        let mut bloom_filter_file_path = PathBuf::new();
        bloom_filter_file_path.push(self.dir.as_path());
        bloom_filter_file_path.push(format!("{}.blmf", active_file.file_id()));


        let mut buffer_file = OpenOptions::new().create(true).write(true).open(buffer_file_path.as_path())?;

        let mut bloom_filter = Bloom::new(20000, max(buffer.capacity(), 10));

        let single_buffer: Vec<_> = buffer.iter().flat_map(|(key, value)| {
            bloom_filter.set(key);
            let data_entry = DataEntry::new(key.clone(), value.clone());
            data_entry.encode()
        }).collect();
        buffer_file.write_all(&single_buffer)?;
        buffer_file.sync_all()?;

        let mut key_entries : BTreeMap<Vec<u8>, KeyDirEntry> = BTreeMap::new();
        for (key, value) in buffer.drain() {
            let data_entry = DataEntry::new(key.clone(), value);
            let key_dir_entry = active_file.write(&data_entry, &self.keys_dir)?;
            key_entries.insert(key, key_dir_entry);
        }
        active_file.sync()?;
        self.keys_dir.insert_bulk(key_entries);
        fs_extra::remove_items(&vec![buffer_file_path.as_path()]);
        if split_active_file {
            self.try_split(&mut active_file)?;
            self.index_dir.insert(active_file.as_file_pair().clone())?;
        }
        self.keys_dir.insert_bloom(file_name.clone(), bloom_filter);
        //println!("Sync Done {} Keys writen to database index id {}", key_size, file_name);
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.keys_dir.size()
    }
}

impl Drop for DataStore {
    fn drop(&mut self) {
        self.sync_all(false);
        self.lock_file.unlock().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use crate::datastore::{DataStore};
    use serial_test::serial;
    use std::sync::Arc;
    use crate::edgekv::DBIterator;

    #[test]
    #[serial]
    fn test_data_store() {
        let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
        ds.put(vec![1, 2, 3], vec![4, 5, 6])
            .unwrap();
        ds.sync_all(true);
        println!("{:#?}", ds.get(&vec![1, 2, 3]).unwrap());
        clean_up()
    }

    #[test]
    #[serial]
    fn test_data_reopens() {
        clean_up();
        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            ds.put(vec![1, 2, 3], vec![4, 5, 6])
                .unwrap();
            ds.put(vec![3, 1, 2], vec![12, 32, 1])
                .unwrap();
            ds.put(vec![3, 1, 4], vec![121, 200, 187])
                .unwrap();
            ds.put(vec![1, 2, 3], vec![3, 3, 3])
                .unwrap();
            println!("{:#?}", ds.keys());
        }

        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            ds.put(vec![1, 2, 3], vec![9, 9, 6])
                .unwrap();
            //ds.delete(&vec![3, 1, 2]).unwrap();
            println!("{:?}", ds.get(&vec![1, 2, 3]));
            println!("{:#?}", ds.keys());
        }

        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            //ds.delete(&vec![1, 2, 3]).unwrap();
            println!("{:#?}", ds.keys());
        }

        clean_up()
    }

    #[test]
    #[serial]
    fn test_data_reopens_prefix() {
        clean_up();
        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            ds.put(vec![1, 2, 3], vec![4, 5, 6])
                .unwrap();
            ds.sync_all(true);
            ds.put(vec![3, 1, 2], vec![12, 32, 1])
                .unwrap();
            ds.sync_all(true);
            ds.put(vec![3, 1, 4], vec![121, 200, 187])
                .unwrap();
            ds.sync_all(true);
            ds.put(vec![1, 3, 3], vec![3, 3, 3])
                .unwrap();
        }

        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            ds.put(vec![1, 2, 3], vec![9, 9, 6])
                .unwrap();
            //ds.delete(&vec![3, 1, 2]).unwrap();
            println!("{:?}", ds.prefix(&vec![1]));
            println!("{:#?}", ds.keys());
        }


        clean_up()
    }

    #[test]
    #[serial]
    fn test_data_gets_reopens() {
        clean_up();
        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            ds.put(vec![1, 2, 3], vec![4, 5, 6])
                .unwrap();
            ds.put(vec![3, 1, 2], vec![12, 32, 1])
                .unwrap();
            ds.put(vec![3, 1, 4], vec![121, 200, 187])
                .unwrap();
            ds.put(vec![1, 2, 3], vec![3, 3, 3])
                .unwrap();
            println!("{:#?}", ds.keys());
        }

        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            println!("{:?}", ds.get(&vec![1, 2, 3]));
            println!("{:?}", ds.get(&vec![3, 1, 2]));
            println!("{:?}", ds.get(&vec![3, 1, 4]));
        }
        clean_up()
    }

    #[test]
    #[serial]
    fn test_data_iterator() {
        clean_up();
        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            ds.put(vec![1, 2, 3], vec![4, 5, 6])
                .unwrap();
            ds.put(vec![3, 1, 2], vec![12, 32, 1])
                .unwrap();
            ds.put(vec![3, 1, 4], vec![121, 200, 187])
                .unwrap();
            ds.put(vec![1, 2, 3], vec![3, 3, 3])
                .unwrap();
            //println!("{:#?}", ds.keys());

            let iter =  DBIterator::new(Arc::new(ds));
            for r in iter {
                println!("{:?}", r);
            }
        }
        clean_up()
    }



    #[test]
    #[serial]
    fn test_multi_thread_reads() {
        clean_up();
        {
            let mut ds = DataStore::open("./testdir/_test_data_store").unwrap();
            ds.put(vec![1, 2, 3], vec![4, 5, 6])
                .unwrap();
            ds.put(vec![3, 1, 2], vec![12, 32, 1])
                .unwrap();
            ds.put(vec![3, 1, 4], vec![121, 200, 187])
                .unwrap();
            ds.put(vec![1, 2, 3], vec![3, 3, 3])
                .unwrap();
            println!("{:#?}", ds.keys());
        }

        {
            let mut ds = Arc::new(DataStore::open("./testdir/_test_data_store").unwrap());
            let mut handles = vec![];
            for i in 0..3 {
                let ds = ds.clone();
                handles.push(std::thread::spawn( move ||{
                    println!("{:?}", ds.get(&vec![1, 2, 3]));
                    println!("{:?}", ds.get(&vec![3, 1, 2]));
                    println!("{:?}", ds.get(&vec![3, 1, 4]));
                    ds.put(vec![1, 2, 3], vec![3, i, 3]).unwrap();
                    ds.put(vec![i, 2, 3], vec![3, i, 3]).unwrap();
                    ds.sync_all(true);
                }))
            }
           for handle in handles {
               handle.join();
           }

           let iter =  DBIterator::new(ds);
            for r in iter {
                println!("{:?}", r);
            }
        }
        clean_up()
    }


    #[test]
    #[serial]
    fn test_data_merge_store() {
        clean_up();
        {
            let mut ds = DataStore::open("./testdir/_test_data_merge_store").unwrap();
            ds.put(vec![1, 2, 3], vec![4, 5, 6])
                .unwrap();
            ds.put(vec![3, 1, 2], vec![12, 32, 1])
                .unwrap();
            ds.put(vec![3, 1, 4], vec![121, 200, 187])
                .unwrap();
            ds.put(vec![1, 2, 3], vec![3, 3, 3])
                .unwrap();
        }

        {
            let mut ds = DataStore::open("./testdir/_test_data_merge_store").unwrap();
            ds.put(vec![1, 2, 3], vec![4, 4, 4])
                .unwrap();
            ds.put(vec![3, 1, 2], vec![12, 32, 1])
                .unwrap();
            ds.put(vec![3, 1, 4], vec![12, 54, 0])
                .unwrap();
            ds.put(vec![8, 27, 34], vec![3, 3, 3])
                .unwrap();
        }

        let mut keys_before_merge = vec![];
        let mut keys_after_merge = vec![];

        {
            let mut ds = DataStore::open("./testdir/_test_data_merge_store").unwrap();
            ds.put(vec![1, 2, 3], vec![9, 9, 6])
                .unwrap();
            ds.delete(&vec![3, 1, 2]).unwrap();
            keys_before_merge = ds.keys();
        }

        {
            let mut ds = DataStore::open("./testdir/_test_data_merge_store").unwrap();
            ds.merge();
            keys_after_merge = ds.keys();
        }

        assert_eq!(keys_before_merge, keys_after_merge);
        clean_up();
    }

    #[test]
    #[serial]
    fn test_reopen_without_closing_error() {
        let mut ds = DataStore::open("./testdir/_test_data_merge_store").unwrap();
        ds.put(vec![1, 2, 3], vec![4, 5, 6])
            .unwrap();
        ds.put(vec![3, 1, 2], vec![12, 32, 1])
            .unwrap();
        ds.put(vec![3, 1, 4], vec![121, 200, 187])
            .unwrap();
        ds.put(vec![1, 2, 3], vec![3, 3, 3])
            .unwrap();
        println!("{:#?}", ds.keys());

        let mut open_result = DataStore::open("./testdir/_test_data_merge_store");
        assert!(open_result.is_err());
    }

    fn clean_up() {
        fs_extra::dir::remove("./testdir");
    }
}
