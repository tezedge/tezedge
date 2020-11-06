// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;

use assert_json_diff::assert_json_eq_no_panic;
use bytes::buf::BufExt;
use enum_iterator::IntoEnumIterator;
use failure::format_err;
use hyper::Client;
use itertools::Itertools;
use rand::prelude::SliceRandom;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, IntoEnumIterator)]
pub enum NodeType {
    Node1,
    Node2,
}

#[ignore]
#[tokio::test]
async fn test_rpc_compare() {
    integration_tests_rpc(from_block_header(), to_block_header()).await
}

async fn integration_tests_rpc(from_block: i64, to_block: i64) {
    let mut cycle_loop_counter: i64 = 0;
    const MAX_CYCLE_LOOPS: i64 = 4;

    for level in from_block..to_block + 1 {
        if level <= 0 {
            test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "header")).await;
            println!("Genesis with level: {:?} - skipping another rpc comparisons for this block", level);
            continue;
        }

        // -------------------------- Integration tests for RPC --------------------------
        // ---------------------- Please keep one function per test ----------------------

        // --------------------------- Tests for each block_id - shell rpcs ---------------------------
        test_rpc_compare_json(&format!("{}/{}", "chains/main/blocks", level)).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "header")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "header/shell")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "hash")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "protocols")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "operation_hashes")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "context/raw/bytes/cycle")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "context/raw/bytes/rolls/owner/current")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "context/raw/bytes/contracts")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "live_blocks")).await;

        // --------------------------- Tests for each block_id - protocol rpcs ---------------------------
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "context/constants")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "helpers/endorsing_rights")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "helpers/baking_rights")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "votes/listings")).await;
        // TODO: TE-226 - uncomment after protocol rpc router is done
        // test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "helpers/current_level")).await;
        // test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "minimal_valid_time")).await;
        // --------------------------------- End of tests --------------------------------

        // we need some constants
        let constants_json = try_get_data_as_json(&format!("{}/{}/{}", "chains/main/blocks", level, "context/constants")).await.expect("Failed to get constants");
        let preserved_cycles = constants_json["preserved_cycles"].as_i64().expect(&format!("No constant 'preserved_cycles' for block_id: {}", level));
        let blocks_per_cycle = constants_json["blocks_per_cycle"].as_i64().expect(&format!("No constant 'blocks_per_cycle' for block_id: {}", level));
        let blocks_per_roll_snapshot = constants_json["blocks_per_roll_snapshot"].as_i64().expect(&format!("No constant 'blocks_per_roll_snapshot' for block_id: {}", level));

        // test last level of snapshot
        if level >= blocks_per_roll_snapshot && level % blocks_per_roll_snapshot == 0 {
            // --------------------- Tests for each snapshot of the cycle ---------------------
            println!("run snapshot tests: level: {:?}, blocks_per_roll_snapshot: {}", level, blocks_per_roll_snapshot);

            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level - 1))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level - 10))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level - 1000))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level - 3000))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level + 1)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level + 10)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level + 1000)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level + 3000)).await;

            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level - 1))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level - 10))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level - 1000))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level - 3000))).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level + 1)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level + 10)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level + 1000)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level + 3000)).await;

            // ----------------- End of tests for each snapshot of the cycle ------------------
        }

        // test first and last level of cycle
        if level == 1 || (level >= blocks_per_cycle && ((level - 1) % blocks_per_cycle == 0 || level % blocks_per_cycle == 0)) {

            // get block cycle
            let cycle: i64 = if level == 1 {
                // block level 1 does not have metadata/level/cycle, so we use 0 instead
                0
            } else {
                let block_json = try_get_data_as_json(&format!("{}/{}", "chains/main/blocks", level)).await.expect("Failed to get block metadata");
                block_json["metadata"]["level"]["cycle"].as_i64().unwrap()
            };

            // ----------------------- Tests for each cycle of the cycle -----------------------
            println!("run cycle tests: {}, level: {}, blocks_per_cycle: {}", cycle, level, blocks_per_cycle);

            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", level, "helpers/endorsing_rights", cycle)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", level, "helpers/endorsing_rights", cycle + preserved_cycles)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, cycle - 2))).await;

            test_rpc_compare_json(&format!("{}/{}/{}?all=true&cycle={}", "chains/main/blocks", level, "helpers/baking_rights", cycle)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?all=true&cycle={}", "chains/main/blocks", level, "helpers/baking_rights", cycle + preserved_cycles)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?all=true&cycle={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, cycle - 2))).await;

            // get all cycles - it is like json: [0,1,2,3,4,5,7,8]
            let cycles = try_get_data_as_json(&format!("chains/main/blocks/{}/context/raw/json/cycle", level)).await.expect("Failed to get cycle data");
            let cycles = cycles.as_array().expect("No cycles data");

            // tests for cycles from protocol/context
            for cycle in cycles {
                let cycle = cycle.as_u64().expect(&format!("Invalid cycle: {}", cycle));
                test_rpc_compare_json(&format!("{}/{}/{}/{}", "chains/main/blocks", level, "context/raw/bytes/cycle", cycle)).await;
                test_rpc_compare_json(&format!("{}/{}/{}/{}", "chains/main/blocks", level, "context/raw/json/cycle", cycle)).await;
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
}

async fn test_rpc_compare_json(rpc_path: &str) {
    let _ = test_rpc_compare_json_and_return_if_eq(rpc_path).await;
}

async fn test_rpc_compare_json_and_return_if_eq(rpc_path: &str) -> (Value, Value) {
    // print the asserted path, to know which one errored in case of an error, use --nocapture
    println!("Checking: {}", rpc_path);
    let node1_json = match get_rpc_as_json(NodeType::Node1, rpc_path).await {
        Ok(json) => json,
        Err(e) => panic!("Failed to call rpc on Node1: '{}', Reason: {}", node_rpc_url(NodeType::Node1, rpc_path), e)
    };
    let node2_json = match get_rpc_as_json(NodeType::Node2, rpc_path).await {
        Ok(json) => json,
        Err(e) => panic!("Failed to call rpc on Node2: '{}', Reason: {}", node_rpc_url(NodeType::Node2, rpc_path), e)
    };
    if let Err(error) = assert_json_eq_no_panic(&node2_json, &node1_json) {
        panic!("\n\nError: \n{}\n\nnode2_json:\n{}\n\nnode1_json:\n{}", error, node2_json, node1_json);
    } else {
        (node1_json, node2_json)
    }
}

/// Returns json data from any/random node (if fails, tries other)
async fn try_get_data_as_json(rpc_path: &str) -> Result<serde_json::value::Value, failure::Error> {
    let mut nodes: Vec<NodeType> = NodeType::into_enum_iter().collect_vec();
    nodes.shuffle(&mut rand::thread_rng());

    for node in nodes {
        match get_rpc_as_json(node.clone(), rpc_path).await {
            Ok(data) => return Ok(data),
            Err(e) => {
                println!("WARN: failed for (node: {:?}) to get data for rpc '{}'. Reason: {}", node.clone(), node_rpc_url(node, rpc_path), e);
            }
        }
    }

    Err(format_err!("No more nodes to choose for rpc call: {}", rpc_path))
}

async fn get_rpc_as_json(node: NodeType, rpc_path: &str) -> Result<serde_json::value::Value, failure::Error> {
    let url_as_string = node_rpc_url(node, rpc_path);
    let url = url_as_string.parse().expect(&format!("Invalid URL: {}", &url_as_string));

    let client = Client::new();
    let body = match client.get(url).await {
        Ok(res) => hyper::body::aggregate(res.into_body()).await.expect("Failed to read response body"),
        Err(e) => return Err(format_err!("Request url: {:?} for getting block failed: {} - please, check node's log, in the case of network or connection error, please, check rpc/README.md for CONTEXT_ROOT configurations", url_as_string, e)),
    };

    Ok(serde_json::from_reader(&mut body.reader())?)
}

fn node_rpc_url(node: NodeType, rpc_path: &str) -> String {
    match node {
        NodeType::Node1 => format!(
            "{}/{}",
            &node_rpc_context_root_1(),
            rpc_path
        ),
        NodeType::Node2 => format!(
            "{}/{}",
            &node_rpc_context_root_2(),
            rpc_path
        ), // Tezedge node
    }
}

fn from_block_header() -> i64 {
    env::var("FROM_BLOCK_HEADER")
        .unwrap_or_else(|_| panic!("FROM_BLOCK_HEADER env variable is missing, check rpc/README.md"))
        .parse()
        .unwrap_or_else(|_| panic!("FROM_BLOCK_HEADER env variable can not be parsed as a number, check rpc/README.md"))
}

fn to_block_header() -> i64 {
    env::var("TO_BLOCK_HEADER")
        .unwrap_or_else(|_| panic!("TO_BLOCK_HEADER env variable is missing, check rpc/README.md"))
        .parse()
        .unwrap_or_else(|_| panic!("TO_BLOCK_HEADER env variable can not be parsed as a number, check rpc/README.md"))
}

fn node_rpc_context_root_1() -> String {
    env::var("NODE_RPC_CONTEXT_ROOT_1")
        .expect("env variable 'NODE_RPC_CONTEXT_ROOT_1' should be set")
}

fn node_rpc_context_root_2() -> String {
    env::var("NODE_RPC_CONTEXT_ROOT_2")
        .expect("env variable 'NODE_RPC_CONTEXT_ROOT_2' should be set")
}