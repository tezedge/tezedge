extern crate reqwest;

mod common;

// 2 nodes(rust,ocaml) must run and be bootstraped before these tests can be executed


#[test]
fn integ_test_connect_rust_node() {

    let resp = reqwest::get("http://127.0.0.1:18732/stats/memory").unwrap();
    assert!(resp.status().is_success())

}

#[test]
fn integ_test_rpc_stats_memory() {

    let json_rust = common::call_rpc_json("http://127.0.0.1:8732/stats/memory".to_string()).unwrap();
    let json_ocaml = common::call_rpc_ssl_json("https://zeronet.simplestaking.com:3000/stats/memory".to_string()).unwrap();

    assert!(common::json_structure_match(&json_rust, &json_ocaml))
}



