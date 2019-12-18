// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::skip_list::{Lane, LEVEL_BASE, TypedLane, SkipListError};
use storage::tests_common::TmpStorage;

use crate::common::Value;

mod common;

#[test]
fn lane_new() {
    let tmp_storage = TmpStorage::create("__lane:lane_new").expect("Storage error");
    let lane = Lane::new(1, 0, tmp_storage.storage().kv());
    assert_eq!(lane.level(), 0);
}

#[test]
fn lane_higher() {
    let tmp_storage = TmpStorage::create("__lane:lane_higher").expect("Storage error");
    let lane = Lane::new(2, 0, tmp_storage.storage().kv());
    let higher_lane = lane.higher_lane();
    assert_eq!(higher_lane.level(), 1);
}

#[test]
fn lane_lower() {
    let tmp_storage = TmpStorage::create("__lane:lane_lower").expect("Storage error");
    let lane = Lane::new(3, 1, tmp_storage.storage().kv());
    let lower_lane = lane.lower_lane();
    assert_eq!(lower_lane.level(), 0);
}

#[test]
fn lane_lower_underflow() {
    let tmp_storage = TmpStorage::create("__lane:lane_lower_underflow").expect("Storage error");
    let lane = Lane::new(4, 0, tmp_storage.storage().kv());
    let lower_lane = lane.lower_lane();
    assert_eq!(lower_lane.level(), 0);
}

#[test]
fn lane_put_get_values() {
    let tmp_storage = TmpStorage::create("__lane:lane_put_get_values").expect("Storage error");
    let lane = Lane::new(5, 0, tmp_storage.storage().kv());
    lane.put(0, &Value::new(vec![0])).expect("failed to put value into lane");
    assert_eq!((lane.get(0) as Result<Option<Value>, SkipListError>).expect("failed to get lane value"), Some(Value::new(vec![0])));
    assert_eq!((lane.get(1) as Result<Option<Value>, SkipListError>).expect("failed to get lane value"), None);
}

#[test]
fn lane_base_iterator() {
    let tmp_storage = TmpStorage::create("__lane:lane_base_iterator").expect("Storage error");
    let lane = Lane::new(6, 0, tmp_storage.storage().kv());
    for x in 0..=10 {
        lane.put(x, &Value::new(vec![x as u64])).expect("failed to put value into lane");
    }

    let mut index = LEVEL_BASE as i64;
    for (k, v) in lane.base_iterator(LEVEL_BASE).expect("Expected valid iterator") {
        let k = k.expect("expected valid header");
        let v: Value = v.expect("expected valid value");
        assert_eq!(k.index(), index as usize);
        assert_eq!(k.level(), 0);
        assert_eq!(v, Value::new(vec![index as u64]));
        index -= 1;
    }
}
