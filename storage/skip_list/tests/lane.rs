// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use skip_list::{Lane, LEVEL_BASE};

use crate::common::{TmpDb, Value};

mod common;

#[test]
fn lane_new() {
    let tmp = TmpDb::new();
    let lane: Lane<Value> = Lane::new(0, tmp.db());
    assert_eq!(lane.level(), 0);
}

#[test]
fn lane_higher() {
    let tmp = TmpDb::new();
    let lane: Lane<Value> = Lane::new(0, tmp.db());
    let higher_lane = lane.higher_lane();
    assert_eq!(higher_lane.level(), 1);
}

#[test]
fn lane_lower() {
    let tmp = TmpDb::new();
    let lane: Lane<Value> = Lane::new(1, tmp.db());
    let lower_lane = lane.lower_lane();
    assert_eq!(lower_lane.level(), 0);
}

#[test]
fn lane_lower_underflow() {
    let tmp = TmpDb::new();
    let lane: Lane<Value> = Lane::new(0, tmp.db());
    let lower_lane = lane.lower_lane();
    assert_eq!(lower_lane.level(), 0);
}

#[test]
fn lane_put_get_values() {
    let tmp = TmpDb::new();
    let lane = Lane::new(0, tmp.db());
    lane.put(0, &Value::new(vec![0])).expect("failed to put value into lane");
    assert_eq!(lane.get(0).expect("failed to get lane value"), Some(Value::new(vec![0])));
    assert_eq!(lane.get(1).expect("failed to get lane value"), None);
}

#[test]
fn lane_base_iterator() {
    let tmp = TmpDb::new();
    let lane = Lane::new(0, tmp.db());
    for x in 0..=10 {
        lane.put(x, &Value::new(vec![x])).expect("failed to put value into lane");
    }

    let mut index = LEVEL_BASE as i64;
    for (k, v) in lane.base_iterator(LEVEL_BASE).expect("Expected valid iterator") {
        let k = k.expect("expected valid header");
        let v = v.expect("expected valid value");
        assert_eq!(k.index(), index as usize);
        assert_eq!(k.level(), 0);
        assert_eq!(v, Value::new(vec![index as usize]));
        index -= 1;
    }
}
