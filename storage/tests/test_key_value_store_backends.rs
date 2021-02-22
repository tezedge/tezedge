// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use rocksdb::Cache;
use rocksdb::{Options, DB};
use std::collections::HashSet;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::{env, fs};
use storage::backend::{BTreeMapBackend, InMemoryBackend, RocksDBBackend, SledBackend};
use storage::merkle_storage::{Entry, EntryHash};
use storage::merkle_storage::{MerkleStorage, MerkleStorageKV};
use storage::persistent::database::KeyValueStoreBackend;
use storage::persistent::KeyValueSchema;
use storage::storage_backend::size_of_vec;

/// Open DB at path, used in tests
fn open_db<P: AsRef<Path>>(path: P, cache: &Cache) -> DB {
    let mut db_opts = Options::default();
    db_opts.create_if_missing(true);
    db_opts.create_missing_column_families(true);

    DB::open_cf_descriptors(&db_opts, path, vec![MerkleStorage::descriptor(&cache)]).unwrap()
}

pub fn out_dir_path(dir_name: &str) -> PathBuf {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    Path::new(out_dir.as_str()).join(Path::new(dir_name))
}

fn get_db_name(db_name: &str) -> PathBuf {
    out_dir_path(db_name)
}

fn get_db(db_name: &str, cache: &Cache) -> DB {
    open_db(get_db_name(db_name), &cache)
}

fn get_storage(backend: &str, db_name: &str, cache: &Cache) -> Box<MerkleStorageKV> {
    match backend {
        "rocksdb" => Box::new(RocksDBBackend::new(Arc::new(get_db(db_name, &cache)))),
        "sled" => Box::new(SledBackend::new(
            sled::Config::new()
                .path(get_db_name(db_name))
                .open()
                .unwrap(),
        )),
        "btree" => Box::new(BTreeMapBackend::new()),
        "inmem" => Box::new(InMemoryBackend::new()),
        _ => {
            panic!("unknown backend set")
        }
    }
}

fn blob(value: Vec<u8>) -> Entry {
    Entry::Blob(value)
}

fn entry_hash(key: &[u8]) -> EntryHash {
    assert!(key.len() < 32);
    let bytes: Vec<u8> = key
        .iter()
        .chain(std::iter::repeat(&0u8))
        .take(32)
        .cloned()
        .collect();

    EntryHash::try_from(bytes).unwrap()
}

fn blob_serialized(value: Vec<u8>) -> Vec<u8> {
    bincode::serialize(&blob(value)).unwrap()
}

fn clean_db(db_name: &str) {
    let _ = DB::destroy(&Options::default(), get_db_name(db_name));
    let _ = fs::remove_dir_all(get_db_name(db_name));
}

fn test_put_get(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_put_get_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    let kv1 = (entry_hash(&[1]), blob_serialized(vec![1]));
    storage.put(&kv1.0, &kv1.1).unwrap();
    let value = storage.get(&kv1.0).unwrap().unwrap();
    assert_eq!(value, kv1.1);
}

fn test_put_twice(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_put_twice_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    storage
        .put(&entry_hash(&[1]), &blob_serialized(vec![11]))
        .unwrap();
    storage
        .put(&entry_hash(&[1]), &blob_serialized(vec![22]))
        .unwrap();
    assert_eq!(
        blob_serialized(vec![22]),
        storage.get(&entry_hash(&[1])).unwrap().unwrap()
    );
}

fn test_put_delete_get(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_put_delete_get_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    let key = entry_hash(&[1]);
    let val1 = blob_serialized(vec![1]);

    storage.put(&key, &val1).unwrap();
    storage.delete(&key).unwrap();

    assert!(storage.get(&key).unwrap().is_none());
}

fn test_contains(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_contains_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    let key = entry_hash(&[1]);
    let val1 = blob_serialized(vec![1]);

    assert!(!storage.contains(&key).unwrap());
    storage.put(&key, &val1).unwrap();
    assert!(storage.contains(&key).unwrap());
}

fn test_delete_non_existing_key(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_delete_non_existing_key_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    let key = entry_hash(&[1]);
    assert!(storage.delete(&key).is_ok());
}

fn test_try_delete_non_existing_key(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_try_delete_non_existing_key_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    let key = entry_hash(&[1]);
    assert!(storage.try_delete(&key).unwrap().is_none());
}

fn test_try_delete_existing_key(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_try_delete_non_existing_key_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);
    let key = entry_hash(&[1]);
    let val1 = blob_serialized(vec![1]);
    storage.put(&key, &val1).unwrap();
    assert_eq!(val1, storage.try_delete(&key).unwrap().unwrap());
}

fn test_write_batch(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_write_batch_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    let batch = vec![
        (entry_hash(&[1]), blob_serialized(vec![11])),
        (entry_hash(&[2]), blob_serialized(vec![22])),
        (entry_hash(&[3]), blob_serialized(vec![33])),
    ];
    storage.write_batch(batch).unwrap();
    assert!(storage.contains(&entry_hash(&[1])).unwrap());
    assert!(storage.contains(&entry_hash(&[2])).unwrap());
    assert!(storage.contains(&entry_hash(&[3])).unwrap());

    assert_eq!(
        blob_serialized(vec![11]),
        storage.get(&entry_hash(&[1])).unwrap().unwrap()
    );
    assert_eq!(
        blob_serialized(vec![22]),
        storage.get(&entry_hash(&[2])).unwrap().unwrap()
    );
    assert_eq!(
        blob_serialized(vec![33]),
        storage.get(&entry_hash(&[3])).unwrap().unwrap()
    );
}

fn test_retain(backend: &str) {
    let cache = Cache::new_lru_cache(32 * 1024 * 1024).unwrap();
    let db_name = &format!("test_retain_{}", backend);
    clean_db(db_name);
    let storage = get_storage(backend, db_name, &cache);

    storage
        .put(&entry_hash(&[1]), &blob_serialized(vec![11]))
        .unwrap();
    storage
        .put(&entry_hash(&[2]), &blob_serialized(vec![22]))
        .unwrap();

    let keys = vec![entry_hash(&[1])].into_iter().collect::<HashSet<_>>();
    storage.retain(&|x| keys.contains(x)).unwrap();

    assert!(storage.get(&entry_hash(&[1])).unwrap().is_some());
    assert!(storage.get(&entry_hash(&[2])).unwrap().is_none());
}

#[test]
fn test_memory_consumption_btree() {
    let entry1 = entry_hash(&[1]);
    let value1 = blob_serialized(vec![1, 2, 3, 3, 5]);
    let entry2 = entry_hash(&[2]);
    let value2 = blob_serialized(vec![11, 22, 33]);

    let storage = BTreeMapBackend::default();
    assert_eq!(0, storage.total_get_mem_usage().unwrap());

    // insert first entry
    storage.put(&entry1, &value1).unwrap();
    assert_eq!(
        std::mem::size_of::<EntryHash>() + size_of_vec(&value1),
        storage.total_get_mem_usage().unwrap()
    );

    // change value under key
    storage.merge(&entry1, &value2).unwrap();
    assert_eq!(
        std::mem::size_of::<EntryHash>() + size_of_vec(&value2),
        storage.total_get_mem_usage().unwrap()
    );

    storage.put(&entry2, &value2).unwrap();
    assert_eq!(
        2 * std::mem::size_of::<EntryHash>() + size_of_vec(&value2) + size_of_vec(&value2),
        storage.total_get_mem_usage().unwrap()
    );

    //remove first entry
    storage.delete(&entry1).unwrap();
    assert_eq!(
        std::mem::size_of::<EntryHash>() + size_of_vec(&value2),
        storage.total_get_mem_usage().unwrap()
    );

    //remove second entry
    storage.delete(&entry2).unwrap();
    assert_eq!(0, storage.total_get_mem_usage().unwrap());
}

#[test]
fn test_memory_consumption_in_memory() {
    let entry1 = entry_hash(&[1]);
    let value1 = blob_serialized(vec![1, 2, 3, 3, 5]);
    let entry2 = entry_hash(&[2]);
    let value2 = blob_serialized(vec![11, 22, 33]);

    let storage = InMemoryBackend::default();
    assert_eq!(0, storage.total_get_mem_usage().unwrap());

    // insert first entry
    storage.put(&entry1, &value1).unwrap();
    assert_eq!(
        std::mem::size_of::<EntryHash>() + size_of_vec(&value1),
        storage.total_get_mem_usage().unwrap()
    );

    // change value under key
    storage.merge(&entry1, &value2).unwrap();
    assert_eq!(
        std::mem::size_of::<EntryHash>() + size_of_vec(&value2),
        storage.total_get_mem_usage().unwrap()
    );

    storage.put(&entry2, &value2).unwrap();
    assert_eq!(
        2 * std::mem::size_of::<EntryHash>() + size_of_vec(&value2) + size_of_vec(&value2),
        storage.total_get_mem_usage().unwrap()
    );

    //remove first entry
    storage.delete(&entry1).unwrap();
    assert_eq!(
        std::mem::size_of::<EntryHash>() + size_of_vec(&value2),
        storage.total_get_mem_usage().unwrap()
    );

    //remove second entry
    storage.delete(&entry2).unwrap();
    assert_eq!(0, storage.total_get_mem_usage().unwrap());
}

macro_rules! test_with_backend {
    ($storage_name:ident, $name_str:expr) => {
        mod $storage_name {
            use serial_test::serial;
            #[test]
            #[serial]
            fn test_put_get() {
                super::test_put_get($name_str)
            }
            #[test]
            #[serial]
            fn test_put_twice() {
                super::test_put_twice($name_str)
            }
            #[test]
            #[serial]
            fn test_put_delete_get() {
                super::test_put_delete_get($name_str)
            }
            #[test]
            #[serial]
            fn test_contains() {
                super::test_contains($name_str)
            }
            #[test]
            #[serial]
            fn test_delete_non_existing_key() {
                super::test_delete_non_existing_key($name_str)
            }

            #[test]
            #[serial]
            fn test_try_delete_non_existing_key() {
                super::test_try_delete_non_existing_key($name_str)
            }
            #[test]
            #[serial]
            fn test_try_delete_existing_key() {
                super::test_try_delete_existing_key($name_str)
            }
            #[test]
            #[serial]
            fn test_write_batch() {
                super::test_write_batch($name_str)
            }
            #[test]
            #[serial]
            fn test_retain() {
                super::test_retain($name_str)
            }
        }
    };
}

test_with_backend!(rocksdb_tests, "rocksdb");
test_with_backend!(sled_tests, "sled");
test_with_backend!(btree_tests, "btree");
test_with_backend!(inmem_tests, "inmem");
