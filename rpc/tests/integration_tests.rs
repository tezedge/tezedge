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
    integration_tests_rpc(125717).await
}

#[ignore]
#[tokio::test]
async fn integration_test_dev() {
    // to execute test run 'cargo test --verbose -- --nocapture --ignored integration_test_dev'
    // start development tests from 1000th block
    integration_tests_rpc(30000).await

}


async fn integration_tests_rpc(start_block: usize) {
    let mut prev_block = start_block;
    let mut shapshot_counter: i32 = 1;
    const SNAPSHOT_LEVEL_COUNT: i32 = 256;
    let mut cycle_loop_counter: i32 = 0;
    const MAX_CYCLE_LOOPS: i32 = 20;
    let mut last_cycle = 0;

    // alocate vector for RPC tests 
    //let mut tasks = Vec::with_capacity(start_block);

    for block_level in (1..=start_block).rev() {
        
        //tasks.push(
             //tokio::spawn(async move {
                let context_raw_bytes_cycle_url = &format!("{}/{}/{}", "chains/main/blocks", block_level, "context/raw/bytes/cycle");
                let context_raw_bytes_rolls_owner_current_url = &format!("{}/{}/{}", "chains/main/blocks", block_level, "context/raw/bytes/rolls/owner/current");                
                test_rpc_compare_json(&context_raw_bytes_rolls_owner_current_url).await;
            //})
        //);
    
    }

    //futures::future::try_join_all(tasks).await;

        // let block_json = get_rpc_as_json(NodeType::Ocaml, &format!("{}/{}", "chains/main/blocks", &prev_block)).await
        //     .expect("Failed to get block from ocaml");
        
        // let prev_block_level: i64 = match block_json["header"]["level"].as_i64() {
        //     Some(value) => value - 1,
        //     None => 0,
        // };
            
        // // Do not check genesys block
        // if prev_block == 0 {
        // // if prev_block == "BLockGenesisGenesisGenesisGenesisGenesisd1f7bcGMoXy" {
        //     println!("Genesis block reached and checked, breaking loop...");
        //     break;
        // }

        // -------------------------- Integration tests for RPC --------------------------
        // ---------------------- Please keep one function per test ----------------------

        // if shapshot_counter == 1 {
        //     let cycle: i64 = block_json["metadata"]["level"]["cycle"].as_i64().unwrap();
        //     let level: i64 = block_json["metadata"]["level"]["level"].as_i64().unwrap();

        //     if last_cycle != cycle {
        //         // ----------------------- Tests for each cycle of the cycle -----------------------
        //         println!("run cycle tests: {}, level: {:?}", cycle, level);

        //         // test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", cycle)).await;
        //         // test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", cycle+2)).await;
        //         // test_rpc_compare_json(&format!("{}/{}/{}?cycle={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, cycle-2) )).await;
        //         // test_rpc_compare_json(&format!("{}/{}/{}?cycle={}&delegate={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", cycle, "tz1YH2LE6p7Sj16vF6irfHX92QV45XAZYHnX")).await;

        //         test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", &prev_block, "context/constants")).await;

        //         // ------------------- End of tests for each cycle of the cycle --------------------
        //     }

        //     // --------------------- Tests for each snapshot of the cycle ---------------------
        //     println!("run snapshot tests: {}, level: {:?}", cycle, level);
        
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-1) )).await;
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-10) )).await;
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-1000) )).await;
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level-10000) )).await;
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level+1) )).await;
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level+10) )).await;
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level+1000) )).await;
        //     // test_rpc_compare_json(&format!("{}/{}/{}?level={}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights", std::cmp::max(0, level+10000) )).await;

        //     // ----------------- End of tests for each snapshot of the cycle ------------------

        //     if cycle_loop_counter == MAX_CYCLE_LOOPS {
        //         break
        //     }
        //     shapshot_counter = SNAPSHOT_LEVEL_COUNT;
        //     cycle_loop_counter += 1;
        //     last_cycle = cycle;
        // } else {
        //     shapshot_counter -= 1;
        // }

        // --------------------------- Tests for each block_id ---------------------------

        // test_rpc_compare_json(&format!("{}/{}", "chains/main/blocks", &prev_block)).await;
        //test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", &prev_block, "helpers/endorsing_rights")).await;
        //test_rpc_compare_json(&format!("{}/{}/{}", "chains/main/blocks", &prev_block, "helpers/baking_rights")).await;

        // --------------------------------- End of tests --------------------------------

    //}    
}

async fn test_rpc_compare_json(rpc_path: &str) {
    // print the asserted path, to know which one errored in case of an error, use --nocapture
    let ocaml_json = get_rpc_as_json(NodeType::Ocaml, rpc_path).await.unwrap();
    let tezedge_json = get_rpc_as_json(NodeType::Tezedge, rpc_path).await.unwrap();
    println!("Checking: {}", rpc_path);
    assert_json_eq!(tezedge_json, ocaml_json);
}

async fn get_rpc_as_json(node: NodeType, rpc_path: &str) -> Result<serde_json::value::Value, serde_json::error::Error> {
    let url = match node {
        NodeType::Ocaml => format!(
            "http://ocaml-node-run:8732/{}",
            //"http://127.0.0.1:8732/{}", //switch for local testing
            //"http://alphanet.simplestaking.com:8732/{}",
            rpc_path
        ), // reference Ocaml node
        NodeType::Tezedge => format!(
            "http://tezedge-node-run:38732/{}",
            //"http://ocaml-node-run:8732/{}", // POW that tests are OK
            //"http://127.0.0.1:18732/{}", //swith for local testing
            //"http://babylon.tezedge.com:38732/{}",
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