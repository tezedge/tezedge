// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use maplit::hashmap;

use storage::tests_common::TmpStorage;
use storage::{
    persistent::StorageType,
    skip_list::{Lane, SkipListError, TryExtend, TypedLane},
};

#[test]
fn lane_new() {
    let tmp_storage = TmpStorage::create("__lane:lane_new").expect("Storage error");
    let lane = Lane::new(
        1,
        0,
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().seq().generator("__lane:lane_new"),
    );
    assert_eq!(lane.level(), 0);
}

#[test]
fn lane_higher() {
    let tmp_storage = TmpStorage::create("__lane:lane_higher").expect("Storage error");
    let lane = Lane::new(
        2,
        0,
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().seq().generator("__lane:lane_higher"),
    );
    let higher_lane = lane.higher_lane();
    assert_eq!(higher_lane.level(), 1);
}

#[test]
fn lane_lower() {
    let tmp_storage = TmpStorage::create("__lane:lane_lower").expect("Storage error");
    let lane = Lane::new(
        3,
        1,
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().seq().generator("__lane:lane_lower"),
    );
    let lower_lane = lane.lower_lane();
    assert_eq!(lower_lane.level(), 0);
}

#[test]
fn lane_lower_underflow() {
    let tmp_storage = TmpStorage::create("__lane:lane_lower_underflow").expect("Storage error");
    let lane = Lane::new(
        4,
        0,
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage
            .storage()
            .seq()
            .generator("__lane:lane_lower_underflow"),
    );
    let lower_lane = lane.lower_lane();
    assert_eq!(lower_lane.level(), 0);
}

#[test]
fn lane_put_get_values() {
    let tmp_storage = TmpStorage::create("__lane:lane_put_get_values").expect("Storage error");
    let mut lane = Lane::new(
        5,
        0,
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage.storage().kv(StorageType::Database),
        tmp_storage
            .storage()
            .seq()
            .generator("__lane:lane_put_get_values"),
    );
    lane.put_list_value(0)
        .expect("failed to extend")
        .try_extend(&hashmap! { 0 => 0 })
        .expect("failed to put value into lane");
    assert_eq!(
        (lane.get_all(0) as Result<Option<Vec<(i32, i32)>>, SkipListError>)
            .expect("failed to get lane value"),
        Some(vec![(0, 0)])
    );
    assert_eq!(
        (lane.get_all(1) as Result<Option<Vec<(i32, i32)>>, SkipListError>)
            .expect("failed to get lane value"),
        None
    );
}
