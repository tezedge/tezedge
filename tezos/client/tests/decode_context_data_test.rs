// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;

use serial_test::serial;

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
#[serial]
fn test_fn_decode_context_data() {
    client::change_runtime_configuration(
        TezosRuntimeConfiguration {
            log_enabled: is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: no_of_ffi_calls_treshold_for_gc(),
        }
    ).unwrap();

    assert_eq!(
        "\"Ps6mwMrF2ER2s51cp9yYpjDcuzQjsc2yAz8bQsRgdaRxw4Fk95H\"".to_string(),
        client::decode_context_data(
            protocol("Ps6mwMrF2ER2s51cp9yYpjDcuzQjsc2yAz8bQsRgdaRxw4Fk95H"),
            key("protocol"),
            data("32227de5351803223564d2f40dbda7fa0fd20682ddfe743d51af3d08f8114273"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"genesis\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, version"),
            data("67656e65736973"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "{\"status\":\"not_running\"}".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("test_chain"),
            data("00"),
        ).unwrap().unwrap()
    );


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

    assert_eq!(
        "12".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, last_block_priority"),
            data("000c"),
        ).unwrap().unwrap()
    );


    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, active_delegates_with_rolls, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, delegates, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, delegates_with_frozen_balance, 61, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_rolls_and_cycles() {
    assert_eq!(
        "0".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, next"),
            data("00000000"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "0".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, limbo"),
            data("00000000"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"p2pk66EmFoQS6b2mYLvCrwjXs7XT1A2znX26HcT9YMiGsyCHyDvsLaF\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, owner, current, 2, 0, 2"),
            data("0202db58471f14e5286a13a30b29c6c685649bfd312e8b80b100a7f1307cabd4ca86"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "797".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, rolls, index, 30, 3, 798, successor"),
            data("0000031d"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, cycle, 0, random_seed"),
            data("0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "1".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, cycle, 0, roll_snapshot"),
            data("0001"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "2700".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, cycle, 0, last_roll, 0"),
            data("00000a8c"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_ramp_up() {
    assert_eq!(
        "[\"16000000\",\"2000000\"]".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, ramp_up, rewards, 5"),
            data("80c8d00780897a"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "[\"512000000\",\"64000000\"]".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, ramp_up, deposits, 16"),
            data("808092f40180a0c21e"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_votes() {
    assert_eq!(
        "\"proposal\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, votes, current_period_kind"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "8000".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, votes, current_quorum"),
            data("00001f40"),
        ).unwrap().unwrap()
    );
}

#[test]
#[serial]
fn test_fn_decode_context_data_contracts() {
    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, global_counter"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"8000000000000\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, balance"),
            data("8080a2a9eae801"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"tz3WXYtyDUNL91qfiCJtVUX746QpNv5i5ve5\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, manager"),
            data("00026fde46af0356a0476dae4e4600172dc9309b3aa4"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"inited\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, spendable"),
            data("696e69746564"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"tz3WXYtyDUNL91qfiCJtVUX746QpNv5i5ve5\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, delegate"),
            data("026fde46af0356a0476dae4e4600172dc9309b3aa4"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"7990000000000\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, change"),
            data("80b8f288c5e801"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "31".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, roll_list"),
            data("0000001f"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "7".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, p256, 6f, de, 46, af, 03, 56a0476dae4e4600172dc9309b3aa4, delegate_desactivation"),
            data("00000007"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, frozen_balance, 0, deposits"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, frozen_balance, 0, fees"),
            data("00"),
        ).unwrap().unwrap()
    );

    assert_eq!(
        "\"0\"".to_string(),
        client::decode_context_data(
            protocol("PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"),
            key("data, contracts, index, ed25519, 89, b5, 12, 22, 97, e589f9ba8b91f4bf74804da2fe8d4a, frozen_balance, 0, rewards"),
            data("00"),
        ).unwrap().unwrap()
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