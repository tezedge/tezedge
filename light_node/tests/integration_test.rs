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

    //maybe we should implement server status RPC call
    let url = format!("{}/{}", rust_node_url, "chains/main/blocks/BLTrP8PxKtE7ajTPGPkeP5iKPs6zZvTMhv7BCVLainLsEeTXLEK");

    let mut bootstrapped = false;
    let mut timeout_counter = 0;
    let step: u32 = 30; //s
    let sleep_duration = Duration::from_secs(step as u64);
    let timeout: u32 = 900; // s
    loop {
        // bootstrap to exact block, try to get block level 269
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
fn integ_test4_rpc_chains_main_blocks_hash() {

    let cfg = common::get_config().unwrap();
    let ocaml_node_url = cfg["ocaml_node_url"].as_str().unwrap();
    let rust_node_url = cfg["rust_node_url"].as_str().unwrap();

    let url_ocaml = format!("{}/{}", ocaml_node_url, "chains/main/blocks/BLTrP8PxKtE7ajTPGPkeP5iKPs6zZvTMhv7BCVLainLsEeTXLEK");
    let url_rust = format!("{}/{}", rust_node_url, "chains/main/blocks/BLTrP8PxKtE7ajTPGPkeP5iKPs6zZvTMhv7BCVLainLsEeTXLEK");

    let json_rust = common::call_rpc_json(&url_ocaml).unwrap();
    let json_ocaml = common::call_rpc_json(&url_rust).unwrap();

    assert_json_eq!(json_rust, json_ocaml);
}
