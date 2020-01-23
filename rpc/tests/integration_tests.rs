// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

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
async fn integration_test_full() {
    // to execute test run 'cargo test --verbose -- --nocapture --ignored integration_test_full'
    // start full test from level 125717
    integration_tests_rpc("BM61Z4zsa8Vu3zF3CYcMa4ZHUfttmJ2eVUavLmC5Lfbnv2dq4Gw").await
}

#[ignore]
#[tokio::test]
async fn integration_test_dev() {
    // to execute test run 'cargo test --verbose -- --nocapture --ignored integration_test_dev'
    // start development tests from 10000th block
    integration_tests_rpc("BM7ujXb2iX5Kvj4zBhCstZysvGryEvp6DEnTRpn1uZ1wSNnSQij").await
}

async fn integration_tests_rpc(start_block: &str) {
    let mut prev_block = start_block.to_string();
    // TODO: take const from constants? from which block?
    const BLOCKS_PER_SNAPSHOT: i64 = 256;
    const BLOCKS_PER_CYCLE: i64 = 2048;
    let mut cycle_loop_counter: i64 = 0;
    const MAX_CYCLE_LOOPS: i64 = 4;

    while prev_block != "" {
        //println!("{}", &format!("{}/{}", "chains/main/blocks", &prev_block));
        let block_json = get_rpc_as_json(NodeType::Ocaml, &format!("{}/{}", "chains/main/blocks", &prev_block)).await
            .expect("Failed to get block from ocaml");
        let predecessor = block_json["header"]["predecessor"]
            .to_string()
            .replace("\"", "");
        // Do not check genesys block
        if prev_block == "BLockGenesisGenesisGenesisGenesisGenesisd1f7bcGMoXy" {
            println!("Genesis block reached and checked, breaking loop...");
            break;
        }

        // -------------------------- Integration tests for RPC --------------------------
        // ---------------------- Please keep one function per test ----------------------

        let level: i64 = block_json["metadata"]["level"]["level"].as_i64().unwrap();
        let cycle: i64 = block_json["metadata"]["level"]["cycle"].as_i64().unwrap();

        // test last level of snapshot
        if level >= BLOCKS_PER_SNAPSHOT && level % BLOCKS_PER_SNAPSHOT == 0 {
            // --------------------- Tests for each snapshot of the cycle ---------------------
            println!("run snapshot tests: {}, level: {:?}", cycle, level);
        
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-1) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-10) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-1000) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-3000) )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", level+1 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", level+10 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", level+1000 )).await;
            test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", level+3000 )).await;

            // ----------------- End of tests for each snapshot of the cycle ------------------
        }

        // test first and last level of cycle
        if level == 1 || (level >= BLOCKS_PER_CYCLE && ( (level-1) % BLOCKS_PER_CYCLE == 0 || level % BLOCKS_PER_CYCLE == 0)) {

            // ----------------------- Tests for each cycle of the cycle -----------------------
            println!("run cycle tests: {}, level: {:?}", cycle, level);

            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", cycle)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", cycle+3)).await;
            test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, cycle-3) )).await;
            //test_rpc_compare_json(&format!("{}/{}/{}?cycle={}&delegate={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", cycle, "tz1YH2LE6p7Sj16vF6irfHX92QV45XAZYHnX")).await;

            test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", &prev_block, "context/constants")).await;

            // ------------------- End of tests for each cycle of the cycle --------------------

            if cycle_loop_counter == MAX_CYCLE_LOOPS*2 {
                break
            }
            cycle_loop_counter += 1;
        }

        // --------------------------- Tests for each block_id ---------------------------

        // test_rpc_compare_json(&format!("{}/{}", "chains/main/blocks", &prev_block)).await;
        test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights")).await;
        //test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", &prev_block, "helpers/baking_rights")).await;

        // --------------------------------- End of tests --------------------------------

        prev_block = predecessor;
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
    let url = match node {
        NodeType::Ocaml => format!(
            "http://ocaml-node-run:8732/{}",
            //"http://127.0.0.1:8732/{}", //switch for local testing
            rpc_path
        ), // reference Ocaml node
        NodeType::Tezedge => format!(
            "http://tezedge-node-run:18732/{}",
            //"http://ocaml-node-run:8732/{}", // POW that tests are OK
            //"http://127.0.0.1:18732/{}", //swith for local testing
            rpc_path
        ), // Tezedge node
    }.parse().expect("Invalid URL");

    let client = Client::new();
    let body = match client.get(url).await {
        Ok(res) => hyper::body::aggregate(res.into_body()).await.expect("Failed to read response body"),
        Err(e) => panic!("Request for getting block failed: {}", e),
    };

    serde_json::from_reader(&mut body.reader())
}