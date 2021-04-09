// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;

use storage::context::kv_store::test_support::{
    blob_serialized, entry_hash, TestContextKvStoreFactoryInstance,
};
use storage::context::kv_store::SupportedContextKeyValueStore;

fn test_put_get(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory.create("test_put_get").unwrap();

    let kv1 = (entry_hash(&[1]), blob_serialized(vec![1]));
    storage.put(&kv1.0, &kv1.1).unwrap();
    let value = storage.get(&kv1.0).unwrap().unwrap();
    assert_eq!(value, kv1.1);
}

fn test_put_twice(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory.create("test_put_twice").unwrap();

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

fn test_put_delete_get(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory.create("test_put_delete_get").unwrap();

    let key = entry_hash(&[1]);
    let val1 = blob_serialized(vec![1]);

    storage.put(&key, &val1).unwrap();
    storage.delete(&key).unwrap();

    assert!(storage.get(&key).unwrap().is_none());
}

fn test_contains(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory.create("test_contains").unwrap();

    let key = entry_hash(&[1]);
    let val1 = blob_serialized(vec![1]);

    assert!(!storage.contains(&key).unwrap());
    storage.put(&key, &val1).unwrap();
    assert!(storage.contains(&key).unwrap());
}

fn test_delete_non_existing_key(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory
        .create("test_delete_non_existing_key")
        .unwrap();

    let key = entry_hash(&[1]);
    assert!(storage.delete(&key).is_ok());
}

fn test_try_delete_non_existing_key(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory
        .create("test_try_delete_non_existing_key")
        .unwrap();

    let key = entry_hash(&[1]);
    assert!(storage.try_delete(&key).unwrap().is_none());
}

fn test_try_delete_existing_key(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory
        .create("test_try_delete_existing_key")
        .unwrap();

    let key = entry_hash(&[1]);
    let val1 = blob_serialized(vec![1]);
    storage.put(&key, &val1).unwrap();
    assert_eq!(val1, storage.try_delete(&key).unwrap().unwrap());
}

fn test_write_batch(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory.create("test_write_batch").unwrap();

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

fn test_retain(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    let storage = kv_store_factory.create("test_retain").unwrap();

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

// TODO: TE-150 - real support mutliprocess
fn test_multiple_open_instances(kv_store_factory: &TestContextKvStoreFactoryInstance) {
    if !kv_store_factory.supports_multiple_opened_instances() {
        return;
    }

    // create main storage instance
    let storage_instance_main = kv_store_factory
        .create("test_multiple_open_instances")
        .unwrap();

    // open second another instances for read
    let storage_instance_1 = kv_store_factory
        .open_readonly_instance("test_multiple_open_instances")
        .unwrap();
    let storage_instance_2 = kv_store_factory
        .open_readonly_instance("test_multiple_open_instances")
        .unwrap();

    // insert data to main
    storage_instance_main
        .put(&entry_hash(&[1]), &blob_serialized(vec![11]))
        .unwrap();
    storage_instance_main
        .put(&entry_hash(&[2]), &blob_serialized(vec![22]))
        .unwrap();

    // read data from main - ok
    assert!(storage_instance_main
        .get(&entry_hash(&[1]))
        .unwrap()
        .is_some());
    assert!(storage_instance_main
        .get(&entry_hash(&[2]))
        .unwrap()
        .is_some());

    // TODO: TE-150 - real support mutliprocess
    // we need to sync with primary
    storage_instance_1
        .sync_with_primary()
        .expect("Failed to sync with primary");
    storage_instance_2
        .sync_with_primary()
        .expect("Failed to sync with primary");

    // read from instance 1/2
    assert!(storage_instance_1.get(&entry_hash(&[1])).unwrap().is_some());
    assert!(storage_instance_1.get(&entry_hash(&[2])).unwrap().is_some());
    assert!(storage_instance_2.get(&entry_hash(&[1])).unwrap().is_some());
    assert!(storage_instance_2.get(&entry_hash(&[2])).unwrap().is_some());
}

macro_rules! tests_with_storage {
    ($storage_tests_name:ident, $kv_store_factory:expr) => {
        mod $storage_tests_name {
            #[test]
            fn test_put_get() {
                super::test_put_get($kv_store_factory)
            }
            #[test]
            fn test_put_twice() {
                super::test_put_twice($kv_store_factory)
            }
            #[test]
            fn test_put_delete_get() {
                super::test_put_delete_get($kv_store_factory)
            }
            #[test]
            fn test_contains() {
                super::test_contains($kv_store_factory)
            }
            #[test]
            fn test_delete_non_existing_key() {
                super::test_delete_non_existing_key($kv_store_factory)
            }
            #[test]
            fn test_try_delete_non_existing_key() {
                super::test_try_delete_non_existing_key($kv_store_factory)
            }
            #[test]
            fn test_try_delete_existing_key() {
                super::test_try_delete_existing_key($kv_store_factory)
            }
            #[test]
            fn test_write_batch() {
                super::test_write_batch($kv_store_factory)
            }
            #[test]
            fn test_retain() {
                super::test_retain($kv_store_factory)
            }
            #[test]
            fn test_multiple_open_instances() {
                super::test_multiple_open_instances($kv_store_factory)
            }
        }
    };
}

lazy_static::lazy_static! {
    static ref SUPPORTED_KV_STORES: std::collections::HashMap<SupportedContextKeyValueStore, TestContextKvStoreFactoryInstance> = storage::context::kv_store::test_support::all_kv_stores(out_dir_path());
}

fn out_dir_path() -> PathBuf {
    let out_dir = env::var("OUT_DIR")
        .expect("OUT_DIR is not defined - please add build.rs to root or set env variable OUT_DIR");
    out_dir.as_str().into()
}

macro_rules! tests_with_all_kv_stores {
    () => {
        tests_with_storage!(
            kv_store_inmemory_tests,
            super::SUPPORTED_KV_STORES
                .get(&storage::context::kv_store::SupportedContextKeyValueStore::InMem)
                .unwrap()
        );
        tests_with_storage!(
            kv_store_btree_tests,
            super::SUPPORTED_KV_STORES
                .get(&storage::context::kv_store::SupportedContextKeyValueStore::BTreeMap)
                .unwrap()
        );
        // TODO - TE-261: remove this one
        tests_with_storage!(
            kv_store_rocksdb_tests,
            super::SUPPORTED_KV_STORES
                .get(
                    &storage::context::kv_store::SupportedContextKeyValueStore::RocksDB {
                        path: super::out_dir_path()
                    }
                )
                .unwrap()
        );
        tests_with_storage!(
            kv_store_sled_tests,
            super::SUPPORTED_KV_STORES
                .get(
                    &storage::context::kv_store::SupportedContextKeyValueStore::Sled {
                        path: super::out_dir_path()
                    }
                )
                .unwrap()
        );
    };
}

tests_with_all_kv_stores!();
