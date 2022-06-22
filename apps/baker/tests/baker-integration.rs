// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

// cargo build -p baker --tests --release
// LD_LIBRARY_PATH=tezos/sys/lib_tezos/artifacts PATH=$PATH:(pwd)/target/release
// ./target/release/deps/basic-????????????????

use std::time::Duration;

use serial_test::serial;

use baker::{
    machine::BakerAction,
    testing_env::{self, accessor, TestResult},
};

#[test]
#[serial]
fn progress_10() {
    testing_env::run(
        "target/testing".into(),
        Duration::from_secs(60),
        |bakers, watcher, id, action| {
            if let BakerAction::ProposalEvent(a) = &action {
                watcher.check_block(&a.block.predecessor, &bakers[id])?;
            }
            bakers[id].dispatch(action);
            let reach_10_level = bakers
                .iter()
                .all(|baker| accessor::level(baker) == Some(10));
            if reach_10_level {
                Ok(TestResult::Terminate)
            } else {
                Ok(TestResult::Continue)
            }
        },
    )
    .expect("test failed");
}

#[test]
#[serial]
fn progress_10_missing_baker() {
    testing_env::run(
        "target/testing".into(),
        Duration::from_secs(60),
        |bakers, watcher, id, action| {
            if id == 0 {
                return Ok(TestResult::Continue);
            }

            if let BakerAction::ProposalEvent(a) = &action {
                watcher.check_block(&a.block.predecessor, &bakers[id])?;
            }
            bakers[id].dispatch(action);
            let reach_10_level = bakers
                .iter()
                .enumerate()
                .all(|(id, baker)| id == 0 || accessor::level(baker) == Some(10));
            if reach_10_level {
                Ok(TestResult::Terminate)
            } else {
                Ok(TestResult::Continue)
            }
        },
    )
    .expect("test failed");
}
