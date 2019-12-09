#[macro_use]
extern crate assert_json_diff;
extern crate reqwest;
use std::thread::sleep;
use std::time::Duration;

mod common;

//use serde_json::Value;

// integration tests are running sequentialy and need to have configured connection to running ocaml and rust tezos nodes:
// cargo test --verbose -- --nocapture --test-threads=1 --ignored integ_test

#[test]
#[ignore]
fn integ_test1_wait_connect_rust_node() {

    let cfg = common::get_config().unwrap();
    let rust_node_url = cfg["rust_node_url"].as_str().unwrap();

    //maybe we should implement server status RPC call
    let url = format!("{}/{}", rust_node_url, "stats/memory");

    let mut connected = false;
    let mut timeout_counter = 0;
    let step: u32 = 30; //s
    let sleep_duration = Duration::from_secs(step as u64);
    let timeout: u32 = 900; // s
    loop {
        match common::call_rpc(&url) {
            Ok(resp) => { 
                if resp.status().is_success() {
                    connected = true;
                    break
                } else {
                    if timeout_counter < timeout {
                        println!("Waiting for RPC server {}s", timeout_counter);
                        timeout_counter += step;
                        sleep(sleep_duration)
                    } else {
                        println!("Error: timeout for RPC server {}s", timeout_counter);
                        break
                    }
                }
            },
            Err(_e) => {
                if timeout_counter < timeout {
                    println!("Waiting for RPC server {}s", timeout_counter);
                    timeout_counter += step;
                    sleep(sleep_duration)
                } else {
                    println!("Error: timeout for RPC server {}s", timeout_counter);
                    break
                }
            }
        }
    };

    assert!(connected)
}

#[test]
#[ignore]
fn integ_test2_wait_bootstrapp_rust_node() {

    let cfg = common::get_config().unwrap();
    let rust_node_url = cfg["rust_node_url"].as_str().unwrap();
    let block_id = cfg["block_id"].as_str().unwrap();

    //maybe we should implement server status RPC call
    let url = format!("{}/{}/{}", rust_node_url, "chains/main/blocks",block_id);

    let mut bootstrapped = false;
    let mut timeout_counter = 0;
    let step: u32 = 30; //s
    let sleep_duration = Duration::from_secs(step as u64);
    let timeout: u32 = 900; // s
    loop {
        // bootstrap to exact block
        let resp = common::call_rpc(&url).unwrap();
        if resp.status().is_success() {
            // let resp_object: Value = resp.json().unwrap();
            // println!("block head {:?}", resp_object.get("hash").unwrap());
            bootstrapped = true;
            break
        } else {
            if timeout_counter < timeout {
                println!("Waiting for bootstrapp server {}s", timeout_counter);
                timeout_counter += step;
                sleep(sleep_duration)
            } else {
                println!("Bootstrapping server timed out {}s", timeout_counter);
                break
            }
        }
    };

    assert!(bootstrapped)
}

#[test]
#[ignore]
fn integ_test3_rpc_stats_memory() {

    let cfg = common::get_config().unwrap();
    let ocaml_node_url = cfg["ocaml_node_url"].as_str().unwrap();
    let rust_node_url = cfg["rust_node_url"].as_str().unwrap();

    let url_ocaml = format!("{}/{}", ocaml_node_url, "stats/memory");
    let url_rust = format!("{}/{}", rust_node_url, "stats/memory");

    let json_rust = common::call_rpc_json(&url_ocaml).unwrap();
    let json_ocaml = common::call_rpc_json(&url_rust).unwrap();

    assert!(common::json_structure_match(&json_rust, &json_ocaml))
}

#[test]
#[ignore]
fn integ_test4_rpc_chains_main_blocks_id() {

    let cfg = common::get_config().unwrap();
    let ocaml_node_url = cfg["ocaml_node_url"].as_str().unwrap();
    let rust_node_url = cfg["ocaml_node_url"].as_str().unwrap(); //TODO: replace for rust_node_url after command available
    let block_id = cfg["block_id"].as_str().unwrap();

    let url_ocaml = format!("{}/{}/{}", ocaml_node_url, "chains/main/blocks", block_id);
    let url_rust = format!("{}/{}/{}", rust_node_url, "chains/main/blocks", block_id);

    let json_rust = common::call_rpc_json(&url_ocaml).unwrap();
    let json_ocaml = common::call_rpc_json(&url_rust).unwrap();

    assert_json_eq!(json_rust, json_ocaml);
}

#[test]
#[ignore]
fn integ_test5_rpc_chains_main_blocks_id_helpers_baking() {

    let cfg = common::get_config().unwrap();
    let ocaml_node_url = cfg["ocaml_node_url"].as_str().unwrap();
    let rust_node_url = cfg["ocaml_node_url"].as_str().unwrap(); //TODO: replace for rust_node_url after command available
    let block_id = cfg["block_id"].as_str().unwrap();

    let url_ocaml = format!("{}/{}/{}/{}", ocaml_node_url, "chains/main/blocks", block_id, "helpers/baking_rights");
    let url_rust = format!("{}/{}/{}/{}", rust_node_url, "chains/main/blocks", block_id, "helpers/baking_rights");

    let json_rust = common::call_rpc_json(&url_ocaml).unwrap();
    let json_ocaml = common::call_rpc_json(&url_rust).unwrap();

    assert_json_eq!(json_rust, json_ocaml);
}

#[test]
#[ignore]
fn integ_test6_rpc_chains_main_blocks_id_helpers_endorsing() {

    let cfg = common::get_config().unwrap();
    let ocaml_node_url = cfg["ocaml_node_url"].as_str().unwrap();
    let rust_node_url = cfg["ocaml_node_url"].as_str().unwrap(); //TODO: replace for rust_node_url after command available
    let block_id = cfg["block_id"].as_str().unwrap();

    let url_ocaml = format!("{}/{}/{}/{}", ocaml_node_url, "chains/main/blocks", block_id, "helpers/endorsing_rights");
    let url_rust = format!("{}/{}/{}/{}", rust_node_url, "chains/main/blocks", block_id, "helpers/endorsing_rights");

    let json_rust = common::call_rpc_json(&url_ocaml).unwrap();
    let json_ocaml = common::call_rpc_json(&url_rust).unwrap();

    assert_json_eq!(json_rust, json_ocaml);
}

#[test]
#[ignore]
fn integ_test7_rpc_chains_main_blocks_id_context_cycle() {

    let cfg = common::get_config().unwrap();
    let ocaml_node_url = cfg["ocaml_node_url"].as_str().unwrap();
    let rust_node_url = cfg["ocaml_node_url"].as_str().unwrap(); //TODO: replace for rust_node_url after command available
    let block_id = cfg["block_id"].as_str().unwrap();
    let cycle = cfg["cycle"].as_str().unwrap();

    let url_ocaml = format!("{}/{}/{}/{}/{}", ocaml_node_url, "chains/main/blocks", block_id, "context/raw/json/cycle", cycle);
    let url_rust = format!("{}/{}/{}/{}/{}", rust_node_url, "chains/main/blocks", block_id, "context/raw/json/cycle", cycle);

    let json_rust = common::call_rpc_json(&url_ocaml).unwrap();
    let json_ocaml = common::call_rpc_json(&url_rust).unwrap();

    assert_json_eq!(json_rust, json_ocaml);
}

#[test]
#[ignore]
fn integ_test8_rpc_chains_main_blocks_id_context_constants() {

    let cfg = common::get_config().unwrap();
    let ocaml_node_url = cfg["ocaml_node_url"].as_str().unwrap();
    let rust_node_url = cfg["ocaml_node_url"].as_str().unwrap(); //TODO: replace for rust_node_url after command available
    let block_id = cfg["block_id"].as_str().unwrap();

    let url_ocaml = format!("{}/{}/{}/{}", ocaml_node_url, "chains/main/blocks", block_id, "context/constants");
    let url_rust = format!("{}/{}/{}/{}", rust_node_url, "chains/main/blocks", block_id, "context/constants");

    let json_rust = common::call_rpc_json(&url_ocaml).unwrap();
    let json_ocaml = common::call_rpc_json(&url_rust).unwrap();

    assert_json_eq!(json_rust, json_ocaml);
}