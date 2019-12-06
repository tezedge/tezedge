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

    let mut connected = false;
    let mut timeout_counter = 0;
    let step: u32 = 30; //s
    let sleep_duration = Duration::from_secs(step as u64);
    let timeout: u32 = 600; // s
    loop {
        match reqwest::get("http://127.0.0.1:18732/stats/memory") { //maybe we should implement server status RPC call
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

    let mut bootstrapped = false;
    let mut timeout_counter = 0;
    let step: u32 = 30; //s
    let sleep_duration = Duration::from_secs(step as u64);
    let timeout: u32 = 600; // s
    loop {
        // bootstrap to exact block, try to get block level 269
        let resp = reqwest::get("http://127.0.0.1:18732/chains/main/blocks/BLTrP8PxKtE7ajTPGPkeP5iKPs6zZvTMhv7BCVLainLsEeTXLEK").unwrap();
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

    let json_rust = common::call_rpc_json("http://127.0.0.1:18732/stats/memory".to_string()).unwrap();
    let json_ocaml = common::call_rpc_json("https://zeronet.simplestaking.com:3000/stats/memory".to_string()).unwrap();

    assert!(common::json_structure_match(&json_rust, &json_ocaml))
}

#[test]
#[ignore]
fn integ_test4_rpc_chains_main_blocks_hash() {

    let json_rust = common::call_rpc_json("http://127.0.0.1:18732/chains/main/blocks/BLTrP8PxKtE7ajTPGPkeP5iKPs6zZvTMhv7BCVLainLsEeTXLEK".to_string()).unwrap();
    //let json_ocaml = common::call_rpc_json("https://zeronet.simplestaking.com:3000/chains/main/blocks/BLTrP8PxKtE7ajTPGPkeP5iKPs6zZvTMhv7BCVLainLsEeTXLEK".to_string()).unwrap();
    let json_ocaml = common::call_rpc_json("http://babylon.tezedge.com:18732/chains/main/blocks/BLTrP8PxKtE7ajTPGPkeP5iKPs6zZvTMhv7BCVLainLsEeTXLEK".to_string()).unwrap();

    assert_json_eq!(json_rust, json_ocaml);
}
