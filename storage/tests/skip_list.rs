// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use maplit::btreemap;
use rand::{
    distributions::{Distribution, Standard},
    seq::SliceRandom,
    Rng,
};
use serde::{Deserialize, Serialize};

use storage::persistent::{BincodeEncoded, StorageType};
use storage::skip_list::{DatabaseBackedSkipList, TypedSkipList};
use storage::tests_common::TmpStorage;

#[derive(PartialEq, Serialize, Deserialize, Clone, Debug)]
enum Operation {
    Set,
    Copy,
    Delete,
}

impl Distribution<Operation> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Operation {
        use Operation::*;
        match rng.gen_range(0, 3) {
            0 => Set,
            1 => Copy,
            _ => Delete,
        }
    }
}

impl BincodeEncoded for Operation {}

#[test]
fn list_new() {
    let tmp_storage = TmpStorage::create("__skip_list:list_new").expect("Storage error");
    let list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            1,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_new"),
        )
        .expect("failed to create skip list"),
    );
    assert_eq!(list.len(), 0);
}

#[test]
fn list_push() {
    let tmp_storage = TmpStorage::create("__skip_list:list_push").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            2,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_push"),
        )
        .expect("failed to create skip list"),
    );
    list.push(&btreemap! { 1 => 1 })
        .expect("failed to push value to skip list");
    assert!(list.contains(0));
}

#[test]
fn list_index_level() {
    type List = DatabaseBackedSkipList;
    for index in 0..7 {
        assert_eq!(List::index_level(index), 0);
    }
    assert_eq!(List::index_level(7), 1);
    assert_eq!(List::index_level(16), 1);
    assert_eq!(List::index_level(552), 3);
}

#[test]
fn list_check_first() {
    let tmp_storage = TmpStorage::create("__skip_list:list_check_first").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            3,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_check_first"),
        )
        .expect("failed to create skip list"),
    );
    list.push(&btreemap! { 1 => 1 })
        .expect("failed to push value to skip list");
    let val = list.get(0).expect("failed to get value from skip list");
    assert_eq!(
        val.is_some(),
        list.contains(0),
        "List `get` and `contains` return inconsistent answers"
    );
    assert!(val.is_some());
    assert_eq!(val.unwrap(), btreemap! { 1 => 1 });
}

#[test]
fn list_check_second() {
    let tmp_storage = TmpStorage::create("__skip_list:list_check_second").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            4,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_check_second"),
        )
        .expect("failed to create skip list"),
    );
    list.push(&btreemap! { 1 => 1 })
        .expect("failed to push value to skip list");
    list.push(&btreemap! { 2 => 2 })
        .expect("failed to push value to skip list");
    let val = list.get(1).expect("failed to get value from skip list");
    assert_eq!(
        val.is_some(),
        list.contains(1),
        "List `get` and `contains` return inconsistent answers"
    );
    assert!(val.is_some());
    assert_eq!(val.unwrap(), (1..=2).map(|i| (i, i)).collect());
}

#[test]
fn list_check_bottom_lane() {
    let tmp_storage =
        TmpStorage::create("__skip_list:list_check_bottom_lane").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            5,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_check_bottom_lane"),
        )
        .expect("failed to create skip list"),
    );
    for index in 0..=6 {
        list.push(&btreemap! { index => index })
            .expect("failed to push value to skip list");
    }
    assert_eq!(list.levels(), 1);
    let val = list.get(6).expect("failed to get value from skip list");
    assert_eq!(
        val.is_some(),
        list.contains(6),
        "List `get` and `contains` return inconsistent answers"
    );
    assert!(val.is_some());
    assert_eq!(val.unwrap(), (0..=6).map(|i| (i, i)).collect());
}

#[test]
pub fn list_check_faster_lane() {
    let tmp_storage =
        TmpStorage::create("__skip_list:list_check_faster_lane").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            6,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_check_faster_lane"),
        )
        .expect("failed to create skip list"),
    );
    for index in 0..=7 {
        list.push(&btreemap! { index => index })
            .expect("failed to push value to skip list");
    }
    assert_eq!(list.levels(), 2);
    let val = list.get(7).expect("failed to get value from skip list");
    assert_eq!(
        val.is_some(),
        list.contains(7),
        "List `get` and `contains` return inconsistent answers"
    );
    assert!(val.is_some());
    assert_eq!(val.unwrap(), (0..=7).map(|i| (i, i)).collect());
}

#[test]
pub fn list_check_lane_traversal() {
    let tmp_storage =
        TmpStorage::create("__skip_list:list_check_lane_traversal").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            7,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_check_lane_traversal"),
        )
        .expect("failed to create skip list"),
    );
    for index in 0..=63 {
        list.push(&btreemap! { index => index })
            .expect("failed to push value to skip list");
    }
    assert_eq!(list.levels(), 3);
    let val = list.get(63).expect("failed to get value from skip list");
    assert_eq!(
        val.is_some(),
        list.contains(63),
        "List `get` and `contains` return inconsistent answers"
    );
    assert!(val.is_some());
    assert_eq!(val.unwrap(), (0..=63).map(|i| (i, i)).collect());
}

#[test]
pub fn list_get_value_by_key() {
    let tmp_storage =
        TmpStorage::create("__skip_list:list_get_value_by_key").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            8,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_get_value_by_key"),
        )
        .expect("failed to create skip list"),
    );
    for index in 0..=63 {
        list.push(&btreemap! { index => index * 5 })
            .expect("failed to push value to skip list");
    }
    assert_eq!(list.levels(), 3);
    let key: i32 = 21;
    let val = list
        .get_key((key - 1) as usize, &key)
        .expect("failed to get value from skip list");
    assert!(val.is_none(), "Key {} should not be found", key);
    let val = list
        .get_key((key + 1) as usize, &key)
        .expect("failed to get value from skip list");
    assert!(val.is_some(), "Key {} not be found", key);
    assert_eq!(val, Some(key * 5), "Invalid value was found")
}

#[test]
pub fn list_get_values_by_prefix() -> Result<(), failure::Error> {
    let tmp_storage =
        TmpStorage::create("__skip_list:list_get_values_by_prefix").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<String, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            9,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_get_values_by_prefix"),
        )
        .expect("failed to create skip list"),
    );

    list.push(&btreemap! { String::from("/system") => 10 })?;
    list.push(&btreemap! { String::from("/system/cpu") => 11 })?;
    list.push(&btreemap! { String::from("/system/cpu/0") => 110 })?;
    list.push(&btreemap! { String::from("/system/cpu/1") => 111 })?;
    list.push(&btreemap! { String::from("/context/user/name") => 31 })?;
    list.push(&btreemap! { String::from("/context/user/email") => 32 })?;
    list.push(&btreemap! { String::from("/context/user") => 3 })?;
    list.push(&btreemap! { String::from("/context/user/name") => 9000 })?;
    list.push(&btreemap! {
        String::from("/system/cpu/3") => 113,
        String::from("/system/cpu/2") => 112,
    })?;
    assert_eq!(list.len(), 9, "Incorrect length");

    let values = list.get_prefix(list.len() - 1, &String::from("/system"))?;
    assert!(values.is_some());
    assert_eq!(
        values,
        Some(btreemap! {
            String::from("/system") => 10,
            String::from("/system/cpu") => 11,
            String::from("/system/cpu/0") => 110,
            String::from("/system/cpu/1") => 111,
            String::from("/system/cpu/3") => 113,
            String::from("/system/cpu/2") => 112,
        })
    );

    let values = list.get_prefix(list.len() - 1, &String::from("/context/user"))?;
    assert!(values.is_some());
    assert_eq!(
        values,
        Some(btreemap! {
            String::from("/context/user") => 3,
            String::from("/context/user/name") => 9000,
            String::from("/context/user/email") => 32,
        })
    );

    Ok(())
}

#[test]
pub fn list_check_lane_order_traversal() {
    let tmp_storage =
        TmpStorage::create("__skip_list:list_check_lane_order_traversal").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            8,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_check_lane_order_traversal"),
        )
        .expect("failed to create skip list"),
    );
    for (value, key) in (0..=63).zip((0..=7).cycle()) {
        let mut map = BTreeMap::new();
        map.insert(key, value);
        list.push(&map)
            .expect("failed to store value into skip list");
    }
    assert_eq!(list.levels(), 3);
    let val = list.get(63).expect("failed to get value from skip list");
    assert_eq!(
        val.is_some(),
        list.contains(63),
        "List `get` and `contains` return inconsistent answers"
    );
    assert!(val.is_some());
    let mut expected = BTreeMap::new();
    for (value, key) in (56..=63).zip(0..=7) {
        expected.insert(key, value);
    }
    assert_eq!(val.unwrap(), expected);
}

#[test]
pub fn list_check_get_key() {
    let tmp_storage = TmpStorage::create("__skip_list:list_check_get_key").expect("Storage error");
    let mut list: Box<dyn TypedSkipList<i32, i32>> = Box::new(
        DatabaseBackedSkipList::new(
            8,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:list_check_get_key"),
        )
        .expect("failed to create skip list"),
    );
    for x in 0..=7 {
        let mut map = BTreeMap::new();
        map.insert(x, x);
        list.push(&map)
            .expect("failed to store value into skip list");
    }
    assert_eq!(list.levels(), 2);
    let val = list.get_key(7, &7);
    assert_eq!(val.unwrap(), Some(7));
    let val = list.get_key(6, &7);
    assert_eq!(val.unwrap(), None);
}

#[test]
pub fn skip_list_simulate_ledger() {
    let tmp_storage =
        TmpStorage::create("__skip_list:skip_list_simulate_ledger").expect("Storage error");
    let list: Box<dyn TypedSkipList<u64, Operation>> = Box::new(
        DatabaseBackedSkipList::new(
            8,
            tmp_storage.storage().kv(StorageType::Database),
            tmp_storage
                .storage()
                .seq()
                .generator("__skip_list:skip_list_simulate_ledger"),
        )
        .expect("failed to create skip list"),
    );
    simulate_ledger(list);
}

fn simulate_ledger(mut list: Box<dyn TypedSkipList<u64, Operation>>) {
    let ledger_size = 1000;
    let operation_count = 100;
    let key_count = 10000;

    let mut rng = rand::thread_rng();

    let mut context_aggregate: BTreeMap<u64, Operation> = Default::default();
    let mut context_snapshots: Vec<BTreeMap<u64, Operation>> = Default::default();

    for _ in 0..ledger_size {
        let mut state: BTreeMap<u64, Operation> = Default::default();

        for _ in 0..rng.gen_range(1, operation_count) {
            let primary_key = rng.gen_range(0, key_count);
            let secondary_key = state
                .keys()
                .map(|v| v.clone())
                .collect::<Vec<u64>>()
                .choose(&mut rng)
                .map(|v| v.clone());

            match rng.gen() {
                Operation::Set => {
                    state.insert(primary_key, rng.gen());
                }
                Operation::Copy => {
                    if let Some(secondary_key) = secondary_key {
                        state.insert(primary_key, state.get(&secondary_key).unwrap().clone());
                    }
                }
                Operation::Delete => {
                    if let Some(secondary_key) = secondary_key {
                        state.remove(&secondary_key);
                    }
                }
            }
        }

        list.push(&state).expect("Failed storing the context");
        context_aggregate.extend(state);
        context_snapshots.push(context_aggregate.clone())
    }

    for index in 0..ledger_size {
        let provided = list
            .get(index)
            .expect(&format!("expected list to contain index {}", index))
            .expect(&format!("expected correct value on index {}", index));
        let expected = context_snapshots
            .get(index)
            .expect("Unable to retrieve stored context");
        assert_eq!(&provided, expected, "Failed at index {}", index);
    }
}
