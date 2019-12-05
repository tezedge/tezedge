extern crate reqwest;
use std::thread::sleep;
use std::time::Duration;

mod common;


#[test]
#[ignore]
fn integ_test_wait_connect_rust_node() {

    let mut connected = false;
    let mut timeout_counter = 0;
    let step: u32 = 30; //s
    let sleep_duration = Duration::from_secs(step as u64);
    let timeout: u32 = 900; // s
    loop {
        match reqwest::get("http://127.0.0.1:18732/stats/memory") { //maybe we should implement server status RPC call
            Ok(resp) => { 
                if resp.status().is_success() {
                    connected = true;
                    break
                } else {
                    if timeout_counter < timeout {
                        timeout_counter += step;
                        println!("Waiting for RPC server {}s", timeout_counter);
                        sleep(sleep_duration)
                    } else {
                        break
                    }
                }
            },
            Err(_e) => {
                if timeout_counter < timeout {
                    timeout_counter += step;
                    println!("Waiting for RPC server {}s", timeout_counter);
                    sleep(sleep_duration)
                } else {
                    break
                }
            }
        }
    };

    assert!(connected)
}


#[test]
#[ignore]
fn integ_test_rpc_stats_memory() {

    let json_rust = common::call_rpc_json("http://127.0.0.1:18732/stats/memory".to_string()).unwrap();
    let json_ocaml = common::call_rpc_ssl_json("https://zeronet.simplestaking.com:3000/stats/memory".to_string()).unwrap();

    assert!(common::json_structure_match(&json_rust, &json_ocaml))
}