// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![allow(unused_imports)]

use crate::tenderbake_new::Timing;

use super::{BakerAction, BakerState, OperationsEventAction};
use crate::{tenderbake_new::OperationSimple, EventWithTime};

#[allow(dead_code)]
fn test_initialized<F>(f: F)
where
    F: FnOnce(BakerState, i32, Vec<(u16, usize)>),
{
    let state = serde_json::from_str::<BakerState>(include_str!("test_data/state.json")).unwrap();
    let tb_state = &state.as_ref().tb_state;
    let tb_config = &state.as_ref().tb_config;

    let level = tb_state.level().unwrap();

    // take delegates for the level
    let validators = {
        tb_config
            .map
            .delegates
            .get(&level)
            .unwrap()
            .clone()
            .into_values()
            .map(|s| (s.0[0], s.0.len()))
    };

    f(state, level, validators.collect())
}
