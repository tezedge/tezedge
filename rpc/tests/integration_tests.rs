// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;

use assert_json_diff::assert_json_eq;
use bytes::buf::BufExt;
use hyper::Client;

#[derive(Debug)]
pub enum NodeType {
    Tezedge,
    Ocaml,
}

#[ignore]
#[tokio::test]
async fn test_rpc_compare() {
    integration_tests_rpc(from_block_header(), to_block_header()).await
}

async fn integration_tests_rpc(from_block: i64, to_block: i64) {
    let mut cycle_loop_counter: i64 = 0;
    const MAX_CYCLE_LOOPS: i64 = 4;
    // const MINIMAL_BOOTSTRAP_LEVEL: i64 = 500;

    // if to_block >= MINIMAL_BOOTSTRAP_LEVEL {
    //     // allways test a block from the first cycle as they are special cases
    //     println!("Running tests for block from cycle 0: ");
    //     test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", "2", "context/constants")).await;
    //     test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", "2", "helpers/endorsing_rights")).await;
    //     test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", "2", "helpers/baking_rights")).await;

    //     test_rpc_compare_json(&format!("{}/{}/{}?all&cycle=0", "chains/main/blocks", "2", "helpers/baking_rights")).await;
    //     test_rpc_compare_json(&format!("{}/{}/{}?cycle=0", "chains/main/blocks", "2", "helpers/endorsing_rights")).await;

    //     test_rpc_compare_json(&format!("{}/{}/{}?level=0", "chains/main/blocks", "2", "helpers/baking_rights")).await;
    //     test_rpc_compare_json(&format!("{}/{}/{}?level=0", "chains/main/blocks", "2", "helpers/endorsing_rights")).await;

    // } else {
    //     panic!("Tests should allways bootstrap to at least level {}", MINIMAL_BOOTSTRAP_LEVEL);
    // }

    for level in from_block..to_block + 1 {
        let block_json = get_rpc_as_json(NodeType::Ocaml, &format!("{}/{}", "chains/main/blocks", level)).await
            .expect("Failed to get block from ocaml");
        
        // -------------------------- Integration tests for RPC --------------------------
        // ---------------------- Please keep one function per test ----------------------

        // --------------------------- Tests for each block_id ---------------------------
        test_rpc_compare_json(&format!("{}/{}", "chains/main/blocks", level)).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "context/constants")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "helpers/endorsing_rights")).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", level, "helpers/baking_rights")).await;
        // TODO: check listing
        // test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", &block_to_check, "votes/listings")).await;
        // --------------------------------- End of tests --------------------------------

        // we need some constants for
        let constants_json = get_rpc_as_json(NodeType::Tezedge, &format!("{}/{}/{}", "chains/main/blocks", level, "context/constants")).await
            .expect("Failed to get constants from tezedge");

        let preserved_cycles = constants_json["preserved_cycles"].as_i64().expect(&format!("No constant 'preserved_cycles' for block_id: {}", level));
        let blocks_per_cycle = constants_json["blocks_per_cycle"].as_i64().expect(&format!("No constant 'blocks_per_cycle' for block_id: {}", level));
        let blocks_per_roll_snapshot = constants_json["blocks_per_roll_snapshot"].as_i64().expect(&format!("No constant 'blocks_per_roll_snapshot' for block_id: {}", level));

        // block on level 1
        let cycle:i64 = if level == 1 {
            0
        } else {
            block_json["metadata"]["level"]["cycle"].as_i64().unwrap()
        };
        //let cycle: i64 = block_json["metadata"]["level"]["cycle"].as_i64().unwrap();

        // test last level of snapshot
        if level >= blocks_per_roll_snapshot && level % blocks_per_roll_snapshot == 0 {
            // --------------------- Tests for each snapshot of the cycle ---------------------
            println!("run snapshot tests: {}, level: {:?}", cycle, level);

            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level-1) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level-10) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level-1000) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, level-3000) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level+1 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level+10 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level+1000 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/endorsing_rights", level+3000 )).await;

            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level-1) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level-10) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level-1000) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, level-3000) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level+1 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level+10 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level+1000 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", level, "helpers/baking_rights", level+3000 )).await;

            // ----------------- End of tests for each snapshot of the cycle ------------------
        }
            // test first and last level of cycle
        if level == 1 || (level >= blocks_per_cycle && ( (level-1) % blocks_per_cycle == 0 || level % blocks_per_cycle == 0)) {

            // ----------------------- Tests for each cycle of the cycle -----------------------
            println!("run cycle tests: {}, level: {:?}", cycle, level);

            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", level, "helpers/endorsing_rights", cycle)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", level, "helpers/endorsing_rights", cycle+preserved_cycles)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", level, "helpers/endorsing_rights", std::cmp::max(0, cycle-2) )).await;

            test_rpc_compare_json(&format!("{}/{}/{}?all&cycle={}", "chains/main/blocks", level, "helpers/baking_rights", cycle)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?all&cycle={}", "chains/main/blocks", level, "helpers/baking_rights", cycle+preserved_cycles)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?all&cycle={}", "chains/main/blocks", level, "helpers/baking_rights", std::cmp::max(0, cycle-2) )).await;
            //test_rpc_compare_json(&format!("{}/{}/{}?cycle={}&delegate={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", cycle, "tz1YH2LE6p7Sj16vF6irfHX92QV45XAZYHnX")).await;

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

            if cycle_loop_counter == MAX_CYCLE_LOOPS*2 {
                break
            }
            cycle_loop_counter += 1;
        }
    }
}

async fn test_rpc_compare_json(rpc_path: &str) {
    // print the asserted path, to know which one errored in case of an error, use --nocapture
    println!("Checking: {}", rpc_path);
    let ocaml_json = get_rpc_as_json(NodeType::Ocaml, rpc_path).await.unwrap();
    let tezedge_json = get_rpc_as_json(NodeType::Tezedge, rpc_path).await.unwrap();
    assert_json_eq!(tezedge_json, ocaml_json);
}

async fn get_rpc_as_json(node: NodeType, rpc_path: &str) -> Result<serde_json::value::Value, serde_json::error::Error> {
    let url_as_string = node_rpc_url(node, rpc_path);
    let url = url_as_string.parse().expect("Invalid URL");

    let client = Client::new();
    let body = match client.get(url).await {
        Ok(res) => hyper::body::aggregate(res.into_body()).await.expect("Failed to read response body"),
        Err(e) => panic!("Request url: {:?} for getting block failed: {}, in the case of network or connection error, please, check rpc/README.md for CONTEXT_ROOT configurations", url_as_string, e),
    };

    serde_json::from_reader(&mut body.reader())
}

fn node_rpc_url(node: NodeType, rpc_path: &str) -> String {
    match node {
        NodeType::Ocaml => format!(
            "{}/{}",
            &ocaml_node_rpc_context_root(),
            rpc_path
        ), // reference Ocaml node
        NodeType::Tezedge => format!(
            "{}/{}",
            &tezedge_node_rpc_context_root(),
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

fn ocaml_node_rpc_context_root() -> String {
    env::var("OCAML_NODE_RPC_CONTEXT_ROOT")
        .unwrap_or("http://ocaml-node-run:8732".to_string())
}

fn tezedge_node_rpc_context_root() -> String {
    env::var("TEZEDGE_NODE_RPC_CONTEXT_ROOT")
        .unwrap_or("http://tezedge-node-run:18732".to_string())
}