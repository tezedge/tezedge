// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Big integration which compares two nodes for the same rpc result
//!
//! usage:
//!
//! ```
//!     IGNORE_PATH_PATTERNS=/context/raw/bytes FROM_BLOCK_HEADER=0 TO_BLOCK_HEADER=8100 NODE_RPC_CONTEXT_ROOT_1=http://127.0.0.1:16732 NODE_RPC_CONTEXT_ROOT_2=http://127.0.0.1:18888 target/release/deps/integration_tests-4a5eeedb180cbb20 --ignored test_rpc_compare -- --nocapture
//! ```

use std::collections::HashSet;
use std::convert::TryInto;
use std::env;
use std::iter::FromIterator;
use std::time::{Duration, Instant};

use anyhow::format_err;
use hyper::body::Buf;
use hyper::{Client, StatusCode};
use itertools::Itertools;
use lazy_static::lazy_static;
use rand::prelude::SliceRandom;
use serde_json::Value;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use url::Url;

use storage::cycle_eras_storage::CycleEra;

lazy_static! {
    static ref IGNORE_PATH_PATTERNS: Vec<String> = ignore_path_patterns();
    static ref IGNORE_JSON_PROPERTIES: Vec<String> = ignore_json_properties();
    static ref NODE_RPC_CONTEXT_ROOT_1: (String, String) = node_rpc_context_root_1();
    static ref NODE_RPC_CONTEXT_ROOT_2: (String, String) = node_rpc_context_root_2();
}

fn client() -> Client<hyper::client::HttpConnector, hyper::Body> {
    Client::new()
}

fn log_settings() {
    println!("========================================");
    println!("Running rpc compare tests with settings:");
    println!("========================================");
    println!(
        "Node1 url: {} - {}",
        &NODE_RPC_CONTEXT_ROOT_1.0, NODE_RPC_CONTEXT_ROOT_1.1
    );
    println!(
        "Node2 url: {} - {}",
        &NODE_RPC_CONTEXT_ROOT_2.0, NODE_RPC_CONTEXT_ROOT_2.1
    );
    println!(
        "IGNORE_PATH_PATTERNS: {:?}",
        IGNORE_PATH_PATTERNS.join(", ")
    );
    println!(
        "IGNORE_JSON_PROPERTIES: {:?}",
        IGNORE_JSON_PROPERTIES.join(", ")
    );
}

#[derive(Debug, Clone, Copy, PartialEq, EnumIter)]
pub enum NodeType {
    Node1,
    Node2,
}

#[ignore]
#[tokio::test]
async fn test_rpc_compare() {
    log_settings();
    integration_tests_rpc(from_block_header(), to_block_header()).await
}

#[ignore]
#[tokio::test]
async fn test_rpc_compare_rights_mainnet() {
    log_settings();
    integration_test_mainnet_rights_rpc(to_block_header()).await
}

fn get_era_from_level(level: i64, eras: &[CycleEra]) -> CycleEra {
    for era in eras {
        let first_level: i64 = (*era.first_level()).into();
        if first_level > level {
            continue;
        } else {
            return era.clone();
        }
    }
    panic!("No matching cycle era found")
}

fn get_era_from_cycle(cycle: i64, eras: &[CycleEra]) -> CycleEra {
    for era in eras {
        let first_cycle: i64 = (*era.first_cycle()).into();
        if first_cycle > cycle {
            continue;
        } else {
            return era.clone();
        }
    }
    panic!("No matching cycle era found")
}

fn get_cycle_from_era(level: i64, era: &CycleEra) -> i64 {
    let first_level: i64 = (*era.first_level()).into();
    let blocks_per_cycle: i64 = (*era.blocks_per_cycle()).into();
    let first_cycle: i64 = (*era.first_cycle()).into();

    (level - first_level) / blocks_per_cycle + first_cycle
}

fn _get_level_position_in_cycle(level: i64, era: &CycleEra) -> i64 {
    let first_level: i64 = (*era.first_level()).into();
    let blocks_per_cycle: i64 = (*era.blocks_per_cycle()).into();

    (level - first_level) % blocks_per_cycle
}

fn get_first_level_in_cycle(cycle: i64, era: &CycleEra) -> i64 {
    let blocks_per_cycle: i64 = (*era.blocks_per_cycle()).into();
    let first_cycle: i64 = (*era.first_cycle()).into();
    let first_level: i64 = (*era.first_level()).into();

    (cycle - first_cycle) * blocks_per_cycle + first_level
}

fn get_last_level_in_cycle(cycle: i64, era: &CycleEra) -> i64 {
    let blocks_per_cycle: i64 = (*era.blocks_per_cycle()).into();
    let first_level: i64 = (*era.first_level()).into();
    let first_cycle: i64 = (*era.first_cycle()).into();

    (cycle - first_cycle) * blocks_per_cycle + first_level + blocks_per_cycle - 1
}

async fn integration_test_mainnet_rights_rpc(head_level: i64 /* , latest_cycle: i64*/) {
    let granada_first_level = 1589248;
    let florence_last_level = 1589247;

    let constants_json = try_get_data_as_json(
        &format!(
            "{}/{}/{}",
            "chains/main/blocks", head_level, "context/constants"
        ),
        false,
    )
    .await
    .expect("Failed to get constants");

    let preserved_cycles = constants_json["preserved_cycles"]
        .as_i64()
        .unwrap_or_else(|| panic!("No constant 'preserved_cycles' for block_id: {}", head_level));

    // try to get cycle eras from tezedge, if we get Some, override the blocks_per_cycle constatn!
    // /dev/chains/:chain_id/blocks/:block_id/cycle_eras
    let cycle_eras: Vec<CycleEra> = serde_json::from_value(
        try_get_data_as_json(
            &format!(
                "{}/{}/{}",
                "dev/chains/main/blocks", head_level, "cycle_eras"
            ),
            true,
        )
        .await
        .expect("Failed to get eras"),
    )
    .expect("failed to deserialize eras");

    let current_era = get_era_from_level(head_level, &cycle_eras);
    let current_cycle = get_cycle_from_era(head_level, &current_era);

    // check the furthest block from head in future that we still have the data to calculate the rights for
    // this also covers the case when we are requesting the last block of a cycle
    println!("\n===Checking furthest future block===");
    let furthest_cycle_future = current_cycle + preserved_cycles;
    let furthest_level_future = get_last_level_in_cycle(furthest_cycle_future, &current_era);

    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks", head_level, "helpers/baking_rights", furthest_level_future
    ))
    .await
    .expect("test failed");

    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks", head_level, "helpers/endorsing_rights", furthest_level_future
    ))
    .await
    .expect("test failed");

    // check the furthest block from head in past that we still have the data to calculate the rights for
    // this also covers the case when we are requesting the first block of a cycle
    println!("\n===Checking furthest past block===");
    let furthest_cycle_past = if current_cycle < preserved_cycles {
        0
    } else {
        current_cycle - preserved_cycles
    };
    let furthest_level_past = get_first_level_in_cycle(
        furthest_cycle_past,
        &get_era_from_cycle(furthest_cycle_past, &cycle_eras),
    );

    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks", head_level, "helpers/baking_rights", furthest_level_past
    ))
    .await
    .expect("test failed");

    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks", head_level, "helpers/endorsing_rights", furthest_level_past
    ))
    .await
    .expect("test failed");

    // test common call on the edge of protocol switch (on florence, requesting next granada block)
    // test common call on the edge of protocol switch (still in florence)
    println!("\n===Checking last florence block===");
    test_rpc_compare_json(&format!(
        "{}/{}/{}",
        "chains/main/blocks", florence_last_level, "helpers/baking_rights"
    ))
    .await
    .expect("test failed");

    test_rpc_compare_json(&format!(
        "{}/{}/{}",
        "chains/main/blocks", florence_last_level, "helpers/endorsing_rights"
    ))
    .await
    .expect("test failed");

    // test common call on the edge of protocol switch (now in granada)
    println!("\n===Checking first granada block===");
    test_rpc_compare_json(&format!(
        "{}/{}/{}",
        "chains/main/blocks", granada_first_level, "helpers/baking_rights"
    ))
    .await
    .expect("test failed");

    test_rpc_compare_json(&format!(
        "{}/{}/{}",
        "chains/main/blocks", granada_first_level, "helpers/endorsing_rights"
    ))
    .await
    .expect("test failed");

    println!("\n===Checking (from florence) first granda block===");
    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks",
        florence_last_level,
        "helpers/baking_rights",
        florence_last_level + 1
    ))
    .await
    .expect("test failed");

    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks",
        florence_last_level,
        "helpers/endorsing_rights",
        florence_last_level + 1
    ))
    .await
    .expect("test failed");

    // test common call on the edge of protocol switch (on granada, requesting previous florence block)
    println!("\n===Checking (from granada) last florence block===");
    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks",
        granada_first_level,
        "helpers/baking_rights",
        granada_first_level - 1
    ))
    .await
    .expect("test failed");

    test_rpc_compare_json(&format!(
        "{}/{}/{}?level={}",
        "chains/main/blocks",
        granada_first_level,
        "helpers/endorsing_rights",
        granada_first_level - 1
    ))
    .await
    .expect("test failed");

    // test the common call (future)
    let blocks_per_cycle: i64 = (*current_era.blocks_per_cycle()).into();

    // 8 cycles
    let offset = preserved_cycles * blocks_per_cycle;
    let start_level = if offset >= head_level {
        1
    } else {
        head_level - offset
    };

    // common call for tested for preserved cycles from head level
    for level in (start_level..head_level).rev() {
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "helpers/baking_rights"
        ))
        .await
        .expect("test failed");

        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "helpers/endorsing_rights"
        ))
        .await
        .expect("test failed");
    }

    // check future cycles (current_cycle + preserved_cycles) + 1 (+1 to include the last cycle as well)
    println!("\n===Checking future cycles===");
    for cycle in current_cycle..current_cycle + preserved_cycles + 1 {
        test_rpc_compare_json(&format!(
            "{}/{}/{}?cycle={}",
            "chains/main/blocks", head_level, "helpers/baking_rights", cycle
        ))
        .await
        .expect("test failed");

        test_rpc_compare_json(&format!(
            "{}/{}/{}?cycle={}",
            "chains/main/blocks", head_level, "helpers/endorsing_rights", cycle
        ))
        .await
        .expect("test failed");
    }

    // check past cycles (current_cycle - preserved_cycles)
    println!("\n===Checking past cycles===");
    for cycle in current_cycle - preserved_cycles..current_cycle {
        test_rpc_compare_json(&format!(
            "{}/{}/{}?cycle={}",
            "chains/main/blocks", head_level, "helpers/baking_rights", cycle
        ))
        .await
        .expect("test failed");

        test_rpc_compare_json(&format!(
            "{}/{}/{}?cycle={}",
            "chains/main/blocks", head_level, "helpers/endorsing_rights", cycle
        ))
        .await
        .expect("test failed");
    }
}

async fn integration_tests_rpc(from_block: i64, to_block: i64) {
    assert!(
        from_block < to_block,
        "from_block({}) should be smaller then to_block({})",
        from_block,
        to_block
    );

    let mut cycle_loop_counter: i64 = 0;
    const MAX_CYCLE_LOOPS: i64 = 4;

    // check genesis
    test_rpc_compare_json("chains/main/blocks/genesis")
        .await
        .expect("test failed");
    test_rpc_compare_json("chains/main/blocks/genesis/header")
        .await
        .expect("test failed");
    test_rpc_compare_json("chains/main/blocks/genesis/metadata")
        .await
        .expect("test failed");
    test_rpc_compare_json("chains/main/blocks/genesis/operations")
        .await
        .expect("test failed");

    // Check blocks endpoint
    // TODO: cannot compare JSONs for this call in particular because head block hash
    // is likely to be different for each node
    // test_rpc_compare_json("chains/main/blocks")
    //     .await
    //     .expect("test failed");

    // Compare with double and trailing slashes
    test_rpc_compare_json("chains/main//blocks/genesis")
        .await
        .expect("test failed");
    test_rpc_compare_json("chains/main/blocks/genesis/")
        .await
        .expect("test failed");
    test_rpc_compare_json("chains/main/blocks/genesis//")
        .await
        .expect("test failed");

    // lets run rpsc, which doeas not depend on block/level
    test_rpc_compare_json("config/network/user_activated_upgrades")
        .await
        .expect("test failed");
    test_rpc_compare_json("config/network/user_activated_protocol_overrides")
        .await
        .expect("test failed");

    // lets iterate whole rps'c
    for level in from_block..to_block + 1 {
        if level <= 0 {
            println!(
                "Genesis with level: {:?} - skipping another rpc comparisons for this block",
                level
            );
            continue;
        }

        // -------------------------- Integration tests for RPC --------------------------
        // ---------------------- Please keep one function per test ----------------------

        // --------------------------- Tests for each block_id - shell rpcs ---------------------------
        test_rpc_compare_json(&format!("chains/main/blocks/{}", level))
            .await
            .expect("test failed");
        test_rpc_compare_json(&format!("chains/main/blocks/{}/header", level))
            .await
            .expect("test failed");
        test_rpc_compare_json(&format!("chains/main/blocks/{}/metadata", level))
            .await
            .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "header/shell"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!("chains/main/blocks/{}/hash", level))
            .await
            .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "protocols"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "operation_hashes"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/raw/bytes/cycle"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/raw/bytes/rolls/owner/current"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/raw/bytes/contracts"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/raw/bytes/delegates"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/raw/bytes/delegates?depth=0"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/raw/bytes/delegates?depth=1"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/raw/bytes/delegates?depth=2"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "live_blocks"
        ))
        .await
        .expect("test failed");

        test_all_operations_for_block(level).await;

        // --------------------------- Tests for each block_id - protocol rpcs ---------------------------
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "context/constants"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "helpers/endorsing_rights"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "helpers/baking_rights"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "helpers/current_level"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "minimal_valid_time"
        ))
        .await
        .expect("test failed");
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "votes/listings"
        ))
        .await
        .expect("test failed");

        // V1 - compatible rpc
        test_rpc_compare_json(&format!(
            "{}/{}/{}",
            "chains/main/blocks", level, "metadata_hash"
        ))
        .await
        .expect("test failed");
        // --------------------------------- End of tests --------------------------------

        // we need some constants
        let constants_json = try_get_data_as_json(
            &format!("{}/{}/{}", "chains/main/blocks", level, "context/constants"),
            false,
        )
        .await
        .expect("Failed to get constants");
        let preserved_cycles = constants_json["preserved_cycles"]
            .as_i64()
            .unwrap_or_else(|| panic!("No constant 'preserved_cycles' for block_id: {}", level));
        let blocks_per_cycle = constants_json["blocks_per_cycle"]
            .as_i64()
            .unwrap_or_else(|| panic!("No constant 'blocks_per_cycle' for block_id: {}", level));
        let blocks_per_roll_snapshot = constants_json["blocks_per_roll_snapshot"]
            .as_i64()
            .unwrap_or_else(|| {
                panic!(
                    "No constant 'blocks_per_roll_snapshot' for block_id: {}",
                    level
                )
            });

        // test last level of snapshot
        if level >= blocks_per_roll_snapshot && level % blocks_per_roll_snapshot == 0 {
            // --------------------- Tests for each snapshot of the cycle ---------------------
            println!(
                "run snapshot tests: level: {:?}, blocks_per_roll_snapshot: {}",
                level, blocks_per_roll_snapshot
            );

            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                std::cmp::max(0, level - 1)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                std::cmp::max(0, level - 10)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                std::cmp::max(0, level - 1000)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                std::cmp::max(0, level - 3000)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                level + 1
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                level + 10
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                level + 1000
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/endorsing_rights",
                level + 3000
            ))
            .await
            .expect("test failed");

            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                std::cmp::max(0, level - 1)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                std::cmp::max(0, level - 10)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                std::cmp::max(0, level - 1000)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                std::cmp::max(0, level - 3000)
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                level + 1
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                level + 10
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                level + 1000
            ))
            .await
            .expect("test failed");
            test_rpc_compare_json(&format!(
                "{}/{}/{}?level={}",
                "chains/main/blocks",
                level,
                "helpers/baking_rights",
                level + 3000
            ))
            .await
            .expect("test failed");

            // ----------------- End of tests for each snapshot of the cycle ------------------
        }

        // test first and last level of cycle
        if level == 1
            || (level >= blocks_per_cycle
                && ((level - 1) % blocks_per_cycle == 0 || level % blocks_per_cycle == 0))
        {
            // get block cycle
            let cycle: i64 = if level == 1 {
                // block level 1 does not have metadata/level/cycle, so we use 0 instead
                0
            } else {
                let block_json = try_get_data_as_json(
                    &format!("{}/{}/{}", "chains/main/blocks", level, "metadata"),
                    false,
                )
                .await
                .expect("Failed to get block metadata");
                cycle_from_metadata(&block_json).expect("failed to get cycle from metadata")
            };

            // ----------------------- Tests for each cycle of the cycle -----------------------
            println!(
                "run cycle tests: {}, level: {}, blocks_per_cycle: {}",
                cycle, level, blocks_per_cycle
            );

            let cycles_to_check: HashSet<i64> = HashSet::from_iter(
                [cycle, cycle + preserved_cycles, std::cmp::max(0, cycle - 2)].to_vec(),
            );

            for cycle_to_check in cycles_to_check {
                test_rpc_compare_json(&format!(
                    "{}/{}/{}?cycle={}",
                    "chains/main/blocks", level, "helpers/endorsing_rights", cycle_to_check
                ))
                .await
                .expect("test failed");

                test_rpc_compare_json(&format!(
                    "{}/{}/{}?all=true&cycle={}",
                    "chains/main/blocks", level, "helpers/baking_rights", cycle_to_check
                ))
                .await
                .expect("test failed");
            }

            // get all cycles - it is like json: [0,1,2,3,4,5,7,8]
            let cycles = try_get_data_as_json(
                &format!("chains/main/blocks/{}/context/raw/json/cycle", level),
                false,
            )
            .await
            .expect("Failed to get cycle data");
            let cycles = cycles.as_array().expect("No cycles data");

            // tests for cycles from protocol/context
            for cycle in cycles {
                let cycle = cycle
                    .as_u64()
                    .unwrap_or_else(|| panic!("Invalid cycle: {}", cycle));
                test_rpc_compare_json(&format!(
                    "{}/{}/{}/{}",
                    "chains/main/blocks", level, "context/raw/bytes/cycle", cycle
                ))
                .await
                .expect("test failed");
                test_rpc_compare_json(&format!(
                    "{}/{}/{}/{}",
                    "chains/main/blocks", level, "context/raw/json/cycle", cycle
                ))
                .await
                .expect("test failed");
            }

            // known ocaml node bugs
            // - endorsing rights: for cycle 0, when requested cycle 4 there should be cycle check error:
            //  [{"kind":"permanent","id":"proto.005-PsBabyM1.seed.unknown_seed","oldest":0,"requested":4,"latest":3}]
            //  instead there is panic on
            //  [{"kind":"permanent","id":"proto.005-PsBabyM1.context.storage_error","missing_key":["cycle","4","last_roll","1"],"function":"get"}]
            // if cycle==0 {
            //     let block_level_1000 = "BM9xFVaVv6mi7ckPbTgxEe7TStcfFmteJCpafUZcn75qi2wAHrC";
            //     test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", block_level_1000, "helpers/endorsing_rights", 4)).await;
            // }
            // - endorsing rights: if there is last level of cycle is not possible to request cycle - PERSERVED_CYCLES
            // test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, cycle-PERSERVED_CYCLES) )).await;

            // ------------------- End of tests for each cycle of the cycle --------------------

            if cycle_loop_counter == MAX_CYCLE_LOOPS * 2 {
                break;
            }
            cycle_loop_counter += 1;
        }
    }

    // get to_block data
    let to_block_json =
        try_get_data_as_json(&format!("chains/main/blocks/{}/hash", to_block), false)
            .await
            .expect("Failed to get block metadata");
    let from_block_json =
        try_get_data_as_json(&format!("chains/main/blocks/{}/hash", from_block), false)
            .await
            .expect("Failed to get block metadata");
    let genesis_block_json = try_get_data_as_json("chains/main/blocks/genesis/hash", false)
        .await
        .expect("Failed to get block metadata");
    let to_block_hash = to_block_json.as_str().unwrap();
    let from_block_hash = from_block_json.as_str().unwrap();
    let genesis_block_hash = genesis_block_json.as_str().unwrap();

    // test get header by block_hash string
    test_rpc_compare_json(&format!("chains/main/blocks/{}/header", to_block_hash))
        .await
        .expect("test failed");

    // simple test for walking on headers (-, ~)
    let max_offset = std::cmp::max(1, std::cmp::min(5, to_block));
    for i in 0..max_offset {
        // ~
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}~{}/header",
            to_block_hash, i
        ))
        .await
        .expect("test failed");
        // -
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}-{}/header",
            to_block_hash, i
        ))
        .await
        .expect("test failed");
    }

    // walking genesis+<offset>
    test_rpc_compare_json(&format!("chains/main/blocks/genesis+{}/header", from_block))
        .await
        .expect("test failed");

    // walking block_hash+0
    test_rpc_compare_json(&format!("chains/main/blocks/{}+0/header", from_block_hash))
        .await
        .expect("test failed");
    // walking block_hash-0
    test_rpc_compare_json(&format!("chains/main/blocks/{}-0/header", to_block_hash))
        .await
        .expect("test failed");

    // simple test for walking on headers (+)
    let max_offset = std::cmp::max(1, std::cmp::min(5, to_block - from_block));
    for i in 0..max_offset {
        // +
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}+{}/header",
            from_block_hash, i
        ))
        .await
        .expect("test failed");
        // + from genesis
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}+{}/header",
            genesis_block_hash, i
        ))
        .await
        .expect("test failed");
    }

    // genesis_hash-0  / genesis_hash+0
    test_rpc_compare_json(&format!(
        "chains/main/blocks/{}+0/header",
        genesis_block_hash
    ))
    .await
    .expect("test failed");
    test_rpc_compare_json(&format!(
        "chains/main/blocks/{}-0/header",
        genesis_block_hash
    ))
    .await
    .expect("test failed");

    // uknown generated hash - test failure
    let random_block_hash: crypto::hash::BlockHash = [3; crypto::hash::HashType::BlockHash.size()]
        .to_vec()
        .try_into()
        .expect("Failed to create BlockHash");

    test_rpc_compare_json(&format!(
        "chains/main/blocks/{}/header",
        random_block_hash.to_base58_check(),
    ))
    .await
    .expect("test failed");

    // future block - test failure
    let current_head_block_level = find_highest_level().await;

    // future block for shell rpc
    test_rpc_compare_json(&format!(
        "chains/main/blocks/{}/header",
        current_head_block_level + 5555,
    ))
    .await
    .expect("test failed");

    // future block for protocol rpc
    test_rpc_compare_json(&format!(
        "chains/main/blocks/{}/context/constants",
        current_head_block_level + 5555,
    ))
    .await
    .expect("test failed");

    // future block for protocol rpc
    test_rpc_compare_json(&format!(
        "chains/main/blocks/{}/helpers/endorsing_rights",
        current_head_block_level + 5555,
    ))
    .await
    .expect("test failed");

    // future block for protocol rpc
    test_rpc_compare_json(&format!(
        "chains/main/blocks/{}/helpers/baking_rights",
        current_head_block_level + 5555,
    ))
    .await
    .expect("test failed");
}

async fn find_highest_level() -> i64 {
    let rpc_path = "chains/main/blocks/head/header";
    let nodes: Vec<NodeType> = NodeType::iter().collect_vec();

    let mut levels = Vec::with_capacity(nodes.len());
    for node in nodes {
        match get_rpc_as_json(node, rpc_path).await {
            Ok((_, data, _)) => levels.push(data["level"].as_i64().expect("Failed to parse level")),
            Err(e) => {
                println!(
                    "WARN: failed for (node: {:?}) to get data for rpc '{}'. Reason: {}",
                    node.clone(),
                    node_rpc_url(node, rpc_path),
                    e
                );
            }
        }
    }

    levels.into_iter().max().unwrap_or(0)
}

async fn test_rpc_compare_json(rpc_path: &str) -> Result<(), anyhow::Error> {
    // print the asserted path, to know which one errored in case of an error, use --nocapture
    println!();
    if is_ignored(&IGNORE_PATH_PATTERNS, rpc_path) {
        println!("Skipping rpc_path check: {}", rpc_path);
        return Ok(());
    } else {
        println!("Checking: {}", rpc_path);
    }

    // get both responses in parallel
    let node1_response = async {
        match get_rpc_as_json(NodeType::Node1, rpc_path).await {
            Ok(response) => Ok(response),
            Err(e) => Err(format_err!(
                "Failed to call rpc on Node1: '{}', Reason: {}",
                node_rpc_url(NodeType::Node1, rpc_path),
                e
            )),
        }
    };
    let node2_response = async {
        match get_rpc_as_json(NodeType::Node2, rpc_path).await {
            Ok(response) => Ok(response),
            Err(e) => Err(format_err!(
                "Failed to call rpc on Node2: '{}', Reason: {}",
                node_rpc_url(NodeType::Node2, rpc_path),
                e
            )),
        }
    };

    // Wait on both them at the same time:
    let (
        (node1_status_code, node1_json, node1_response_time),
        (node2_status_code, node2_json, node2_response_time),
    ) = futures::try_join!(node1_response, node2_response)?;

    // check jsons
    if let Err(error) =
        json_compare::assert_json_eq_no_panic(&node2_json, &node1_json, &IGNORE_JSON_PROPERTIES)
    {
        panic!(
            "\n\nError: \n{}\n\nnode2_json: ({}, status_code: {})\n{}\n\nnode1_json: ({}, status_code: {})\n{}",
            error,
            node_rpc_url(NodeType::Node2, rpc_path),
            node2_status_code,
            node2_json,
            node_rpc_url(NodeType::Node1, rpc_path),
            node1_status_code,
            node1_json,
        );
    }

    // check status codes
    if node2_status_code.ne(&node1_status_code) {
        panic!(
            "\n\nStatusCodes mismatch: \n({}, status_code: {})\n({}, status_code: {})",
            node_rpc_url(NodeType::Node2, rpc_path),
            node2_status_code,
            node_rpc_url(NodeType::Node1, rpc_path),
            node1_status_code,
        );
    }

    println!(
        "Checked OK: {}, {}: {:?} vs {}: {:?}",
        rpc_path,
        node_name(NodeType::Node1),
        node1_response_time,
        node_name(NodeType::Node2),
        node2_response_time,
    );

    Ok(())
}

/// Returns json data from any/random node (if fails, tries other)
async fn try_get_data_as_json(
    rpc_path: &str,
    use_first_node: bool,
) -> Result<serde_json::value::Value, anyhow::Error> {
    let mut nodes: Vec<NodeType> = NodeType::iter().collect_vec();

    if !use_first_node {
        nodes.shuffle(&mut rand::thread_rng());
    }

    for node in nodes {
        println!("Sending rpc to: {}", node_rpc_url(node, rpc_path));
        match get_rpc_as_json(node, rpc_path).await {
            Ok((_, data, _)) => return Ok(data),
            Err(e) => {
                println!(
                    "WARN: failed for (node: {:?}) to get data for rpc '{}'. Reason: {}",
                    node.clone(),
                    node_rpc_url(node, rpc_path),
                    e
                );
            }
        }
    }

    Err(format_err!(
        "No more nodes to choose for rpc call: {}",
        rpc_path
    ))
}

async fn get_rpc_as_json(
    node: NodeType,
    rpc_path: &str,
) -> Result<(StatusCode, serde_json::value::Value, Duration), anyhow::Error> {
    let url_as_string = node_rpc_url(node, rpc_path);
    let url = url_as_string
        .parse()
        .unwrap_or_else(|_| panic!("Invalid URL: {}", &url_as_string));

    // we create client for every call, because with new Tezos rpcs with "transfer-encoding: chunked"
    // with one singleton Client, calls randomly failed: "connection closed before message completed"
    // maybe it has something to do with keep-alive or something
    // see below [test_chunked_call]
    let client = client();
    let start = Instant::now();
    let (status_code, body, response_time) = match client.get(url).await {
        Ok(res) => {
            let finished = start.elapsed();
            (
                res.status(),
                hyper::body::aggregate(res.into_body()).await.expect("Failed to read response body"),
                finished,
            )
        },
        Err(e) => return Err(format_err!("Request url: {:?} for getting data failed: {} - please, check node's log, in the case of network or connection error, please, check rpc/README.md for CONTEXT_ROOT configurations", url_as_string, e)),
    };

    // process response body
    let mut buf = body.reader();
    let mut dst = vec![];
    std::io::copy(&mut buf, &mut dst).unwrap();

    // process status code
    if status_code.is_success() {
        let response_value = match serde_json::from_slice(&dst) {
            Ok(result) => result,
            Err(err) => {
                return Err(format_err!(
                    "Error {:?} when parsing value as JSON: {:?}",
                    err,
                    String::from_utf8_lossy(&dst)
                ))
            }
        };

        Ok((status_code, response_value, response_time))
    } else {
        let response_value = match serde_json::from_slice(&dst) {
            Ok(result) => result,
            Err(err) => {
                println!(
                    "Error {:?} when parsing value as JSON: {:?}",
                    err,
                    String::from_utf8_lossy(&dst)
                );
                serde_json::Value::Null
            }
        };

        Ok((status_code, response_value, response_time))
    }
}

fn cycle_from_metadata(block_metadata_json: &Value) -> Result<i64, anyhow::Error> {
    // before 008 edo
    if let Some(cycle) = block_metadata_json["level"]["cycle"].as_i64() {
        return Ok(cycle);
    }
    // from 008 edo protocol
    if let Some(cycle) = block_metadata_json["level_info"]["cycle"].as_i64() {
        return Ok(cycle);
    }
    Err(format_err!(
        "No 'cycle' attribute found in block metadata: {:?}",
        block_metadata_json
    ))
}

fn node_rpc_url(node: NodeType, rpc_path: &str) -> String {
    match node {
        NodeType::Node1 => format!("{}/{}", &NODE_RPC_CONTEXT_ROOT_1.0.as_str(), rpc_path),
        NodeType::Node2 => format!("{}/{}", &NODE_RPC_CONTEXT_ROOT_2.0.as_str(), rpc_path),
    }
}

fn node_name(node: NodeType) -> &'static str {
    match node {
        NodeType::Node1 => &NODE_RPC_CONTEXT_ROOT_1.1,
        NodeType::Node2 => &NODE_RPC_CONTEXT_ROOT_2.1,
    }
}

fn from_block_header() -> i64 {
    env::var("FROM_BLOCK_HEADER")
        .unwrap_or_else(|_| {
            panic!("FROM_BLOCK_HEADER env variable is missing, check rpc/README.md")
        })
        .parse()
        .unwrap_or_else(|_| {
            panic!(
                "FROM_BLOCK_HEADER env variable can not be parsed as a number, check rpc/README.md"
            )
        })
}

fn to_block_header() -> i64 {
    env::var("TO_BLOCK_HEADER")
        .unwrap_or_else(|_| panic!("TO_BLOCK_HEADER env variable is missing, check rpc/README.md"))
        .parse()
        .unwrap_or_else(|_| {
            panic!(
                "TO_BLOCK_HEADER env variable can not be parsed as a number, check rpc/README.md"
            )
        })
}

fn ignore_path_patterns() -> Vec<String> {
    env::var("IGNORE_PATH_PATTERNS").map_or_else(
        |_| vec![],
        |paths| {
            paths
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        },
    )
}

fn ignore_json_properties() -> Vec<String> {
    env::var("IGNORE_JSON_PROPERTIES").map_or_else(
        |_| vec![],
        |paths| {
            paths
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        },
    )
}

fn is_ignored(ignore_patters: &[String], rpc_path: &str) -> bool {
    if ignore_patters.is_empty() {
        return false;
    }

    ignore_patters
        .iter()
        .any(|ignored| rpc_path.contains(ignored))
}

fn node_rpc_context_root_1() -> (String, String) {
    let node_url = env::var("NODE_RPC_CONTEXT_ROOT_1")
        .expect("env variable 'NODE_RPC_CONTEXT_ROOT_1' should be set");
    let url = Url::parse(&node_url).expect("invalid url");
    (node_url, url.host_str().unwrap_or("node2").to_string())
}

fn node_rpc_context_root_2() -> (String, String) {
    let node_url = env::var("NODE_RPC_CONTEXT_ROOT_2")
        .expect("env variable 'NODE_RPC_CONTEXT_ROOT_2' should be set");
    let url = Url::parse(&node_url).expect("invalid url");
    (node_url, url.host_str().unwrap_or("node2").to_string())
}

async fn test_all_operations_for_block(level: i64) {
    test_rpc_compare_json(&format!("chains/main/blocks/{}/operations", level))
        .await
        .expect("test failed");

    let validation_passes =
        try_get_data_as_json(&format!("chains/main/blocks/{}/operations", level), false)
            .await
            .expect("Failed to get operations (validation passes)");
    let validation_passes = validation_passes
        .as_array()
        .expect("Failed to parse block operations (validation passes)");

    if !validation_passes.is_empty() {
        // V1 - compatible rpc
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}/operations_metadata_hash",
            level,
        ))
        .await
        .expect("test failed");

        // V1 - compatible rpc
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}/operation_metadata_hashes",
            level,
        ))
        .await
        .expect("test failed");
    }

    for (validation_pass_index, validation_pass) in validation_passes.iter().enumerate() {
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}/operations/{}",
            level, validation_pass_index,
        ))
        .await
        .expect("test failed");

        // V1 - compatible rpc
        test_rpc_compare_json(&format!(
            "chains/main/blocks/{}/operation_metadata_hashes/{}",
            level, validation_pass_index
        ))
        .await
        .expect("test failed");

        let operations = validation_pass
            .as_array()
            .expect("Failed to parse validation pass operations");
        for (operation_index, _) in operations.iter().enumerate() {
            test_rpc_compare_json(&format!(
                "chains/main/blocks/{}/operations/{}/{}",
                level, validation_pass_index, operation_index,
            ))
            .await
            .expect("test failed");

            // V1 - compatible rpc
            test_rpc_compare_json(&format!(
                "chains/main/blocks/{}/operation_metadata_hashes/{}/{}",
                level, validation_pass_index, operation_index
            ))
            .await
            .expect("test failed");
        }
    }
}

#[test]
fn test_ignored_matching() {
    assert!(is_ignored(
        &["minimal_valid_time".to_string()],
        "/chains/main/blocks/1/minimal_valid_time",
    ));
    assert!(is_ignored(
        &["/minimal_valid_time".to_string()],
        "/chains/main/blocks/1/minimal_valid_time",
    ));
    assert!(is_ignored(
        &["1/minimal_valid_time".to_string()],
        "/chains/main/blocks/1/minimal_valid_time",
    ));
    assert!(is_ignored(
        &["vote/listing".to_string()],
        "/chains/main/blocks/1/vote/listing",
    ));
    assert!(!is_ignored(
        &["vote/listing".to_string()],
        "/chains/main/blocks/1/votesasa/listing",
    ));
}

// clear && NODE_RPC_CONTEXT_ROOT_1=http://master.dev.tezedge.com:18733 cargo test --release test_chunked_call -- --nocapture
// #[tokio::test]
// async fn test_chunked_call() {
//     for i in 0..50 {
//         let response = get_rpc_as_json(
//             NodeType::Node1,
//             "/chains/main/blocks/head/helpers/endorsing_rights",
//         )
//         .await
//         .expect("endorsing_rights failed");
//         println!("\n\n{:?}", response);
//         let response = get_rpc_as_json(
//             NodeType::Node1,
//             "/chains/main/blocks/head/helpers/baking_rights",
//         )
//         .await
//         .expect("baking_rights failed");
//         println!("\n\n{:?}", response);
//         let response = get_rpc_as_json(
//             NodeType::Node1,
//             "/chains/main/blocks/head/minimal_valid_time",
//         )
//         .await
//         .expect("minimal_valid_time failed");
//         println!("\n\n{:?}", response);
//     }
// }

mod json_compare {
    use serde::Serialize;

    pub(crate) fn assert_json_eq_no_panic<Lhs, Rhs>(
        lhs: &Lhs,
        rhs: &Rhs,
        ignore_json_properties: &[String],
    ) -> Result<(), String>
    where
        Lhs: Serialize,
        Rhs: Serialize,
    {
        // TODO: hack comparision, because of Tezos bug: https://gitlab.com/tezos/tezos/-/issues/1430
        if !ignore_json_properties.is_empty() {
            assert_json_eq_no_panic_with_ignore_json_properties(
                lhs,
                rhs,
                assert_json_diff::Config::new(assert_json_diff::CompareMode::Strict),
                ignore_json_properties,
            )
        } else {
            assert_json_diff::assert_json_matches_no_panic(
                lhs,
                rhs,
                assert_json_diff::Config::new(assert_json_diff::CompareMode::Strict),
            )
        }
    }

    fn assert_json_eq_no_panic_with_ignore_json_properties<Lhs, Rhs>(
        lhs: &Lhs,
        rhs: &Rhs,
        config: assert_json_diff::Config,
        ignore_json_properties: &[String],
    ) -> Result<(), String>
    where
        Lhs: Serialize,
        Rhs: Serialize,
    {
        let lhs = serde_json::to_value(lhs).unwrap_or_else(|err| {
            panic!(
                "Couldn't convert left hand side value to JSON. Serde error: {}",
                err
            )
        });
        let rhs = serde_json::to_value(rhs).unwrap_or_else(|err| {
            panic!(
                "Couldn't convert right hand side value to JSON. Serde error: {}",
                err
            )
        });

        let diffs = assert_json_diff::diff::diff(&lhs, &rhs, config);

        if diffs.is_empty() {
            Ok(())
        } else {
            let diffs = diffs
                .into_iter()
                .filter(|diff| {
                    if let assert_json_diff::diff::Path::Keys(keys) = &diff.path {
                        if let Some(last_key) = keys.last() {
                            let ignore = ignore_json_properties.iter().any(|ijp| {
                                if let assert_json_diff::diff::Key::Field(field_name) = last_key {
                                    field_name.eq(ijp)
                                } else {
                                    false
                                }
                            });
                            if ignore {
                                println!();
                                println!(
                                    "Found IGNORED property, which does not matches, diff: {:?}",
                                    diff
                                );
                                false
                            } else {
                                true
                            }
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                })
                .collect::<Vec<_>>();

            if diffs.is_empty() {
                Ok(())
            } else {
                let msg = diffs
                    .into_iter()
                    .map(|d| d.to_string())
                    .collect::<Vec<_>>()
                    .join("\n\n");
                Err(msg)
            }
        }
    }
}
