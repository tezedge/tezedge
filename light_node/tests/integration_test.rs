mod common;

// 2 nodes(rust,ocaml) must run and be bootstraped before these tests can be executed

#[test]
#[ignore]
fn integ_rpc_stats_memory() {

    let uri1 = "http://127.0.0.1:18732/stats/memory".to_string(); // local rust node
    let uri2 = "http://127.0.0.1:8732/stats/memory".to_string(); // local ocaml node

    assert!(common::compare_json_structure_of_rpc(uri1, uri2).unwrap())
}

