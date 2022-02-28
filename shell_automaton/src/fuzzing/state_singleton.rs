// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use once_cell::sync::Lazy;
use std::{convert::TryInto, env, io::Read, sync::RwLock};
use storage::persistent::Decoder;

use crate::State;

pub struct FuzzerState {
    pub initial_target_state: State,
    pub current_target_state: State,
    pub iteration_count: u64,
    pub reset_count: u64,
}

pub static FUZZER_STATE: Lazy<RwLock<FuzzerState>> = Lazy::new(|| RwLock::new(initial_state()));

fn initial_state() -> FuzzerState {
    let reset_count = match env::var("STATE_RESET_COUNT") {
        Ok(count) => count.parse().unwrap(),
        _ => 1u64,
    };

    let url = env::var("STATE_SNAPSHOT_URL").unwrap_or(String::from(
        "http://127.0.0.1:18732/dev/shell/automaton/state_raw",
    ));

    println!("Fetching state from: {}", url);

    let resp = match ureq::get(&url).call() {
        Ok(resp) => resp,
        Err(err) => {
            println!("Fetching state failed {:?}", err);
            std::process::exit(1);
        }
    };

    assert!(resp.has("Content-Length"));
    let len: usize = resp.header("Content-Length").unwrap().parse().unwrap();

    println!("Current state size: {} bytes", len);
    let mut bytes: Vec<u8> = Vec::with_capacity(len);

    resp.into_reader()
        .take(len.try_into().unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), len);

    let state = State::decode(&bytes).unwrap();

    FuzzerState {
        initial_target_state: state.clone(),
        current_target_state: state,
        iteration_count: 0,
        reset_count,
    }
}
