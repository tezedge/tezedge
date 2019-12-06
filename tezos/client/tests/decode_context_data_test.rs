// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;

use crypto::hash::{HashType, ProtocolHash};
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_client::client;

fn protocol(hash: &str) -> ProtocolHash {
    HashType::ProtocolHash
        .string_to_bytes(hash)
        .unwrap()
}

fn data(data_as_hex: &str) -> Vec<u8> {
    hex::decode(data_as_hex).unwrap()
}

fn key(key_as_string: &str) -> Vec<String> {
    let key: Vec<&str> = key_as_string.split(", ").collect();
    key
        .iter()
        .map(|k| k.to_string())
        .collect()
}

#[test]
fn test_fn_decode_context_data() {
    client::change_runtime_configuration(
        TezosRuntimeConfiguration {
            log_enabled: is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: no_of_ffi_calls_treshold_for_gc(),
        }
    ).unwrap();

    assert_eq!(
        "1".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, v1, first_level"),
            data("00000001"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"42065708404\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, commitments, 6c, 00, 4d, 09, b8, 9efefb1abbc3555781ffa2ffa57e29"),
            data("f48abfda9c01"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "{\"preserved_cycles\":3,\"blocks_per_cycle\":2048,\"blocks_per_voting_period\":8192,\"time_between_blocks\":[\"30\",\"40\"]}".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, v1, constants"),
            data("ff03ff000008000000ff00002000ff00000010000000000000001e00000000000000280000000000000000000000000000"),
        ).unwrap().unwrap()
    );

    // should fail, som return None
    assert!(
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, v1, constants"),
            data("f48abfda9c01"),
        ).unwrap().is_none()
    );
}

fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>().unwrap()
}

fn no_of_ffi_calls_treshold_for_gc() -> i32 {
    env::var("OCAML_CALLS_GC")
        .unwrap_or("2000".to_string())
        .parse::<i32>().unwrap()
}