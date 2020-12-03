// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use crypto::hash::HashType;
use serial_test::serial;
use tezos_api::environment::{self, TezosEnvironment};
use tezos_api::ffi::{InitProtocolContextResult, TezosRuntimeConfiguration};
use tezos_client::client;
use tezos_interop::ffi;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::BlockHeader;

mod common;

#[test]
#[serial]
fn test_init_protocol_context() {
    // change cfg
    ffi::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        log_enabled: common::is_ocaml_log_enabled(),
    })
    .unwrap();

    let context_hash_encoding = HashType::ContextHash;
    let storage_dir = "test_storage_01";
    let tezos_env = TezosEnvironment::Carthagenet;

    // init empty storage for test WITH commit genesis
    let InitProtocolContextResult {
        genesis_commit_hash,
        supported_protocol_hashes,
    } = prepare_protocol_context(storage_dir, &tezos_env, true);

    // check
    assert!(!supported_protocol_hashes.is_empty());
    assert!(genesis_commit_hash.is_some());
    let genesis_commit_hash = genesis_commit_hash.unwrap();
    assert_eq!(
        context_hash_encoding
            .hash_to_b58check(&genesis_commit_hash)
            .as_str(),
        "CoWZVRSM6DdNUpn3mamy7e8rUSxQVWkQCQfJBg7DrTVXUjzGZGCa",
    );

    // init the same storage without commit
    let InitProtocolContextResult {
        genesis_commit_hash,
        supported_protocol_hashes,
    } = prepare_protocol_context(storage_dir, &tezos_env, false);

    // check
    assert!(!supported_protocol_hashes.is_empty());
    assert!(genesis_commit_hash.is_none());
}

#[test]
#[serial]
fn test_assert_encoding_for_protocol_data() {
    // prepare data
    let block_header_1 = BlockHeader::from_bytes(
        hex::decode(include_str!("resources/block_header_level1.bytes")).unwrap(),
    )
    .expect("Failed to decode block header 1");

    let block_header_2 = BlockHeader::from_bytes(
        hex::decode("0000000201dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d000000005c017cd804683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d0000001100000001000000000800000000000000029bd8c75dec93c276d2d8e8febc3aa6c9471cb2cb42236b3ab4ca5f1f2a0892f6000500000003ba671eef00d6a8bea20a4677fae51268ab6be7bd8cfc373cd6ac9e0a00064efcc404e1fb39409c5df255f7651e3d1bb5d91cb2172b687e5d56ebde58cfd92e1855aaafbf05").unwrap(),
    ).expect("Failed to decode block header 2");

    let protocol_hash_1 = HashType::ProtocolHash
        .b58check_to_hash("PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex")
        .expect("Failed to decode protocol hash");
    let protocol_hash_2 = HashType::ProtocolHash
        .b58check_to_hash("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS")
        .expect("Failed to decode protocol hash");

    // check
    assert!(client::assert_encoding_for_protocol_data(
        protocol_hash_1.clone(),
        block_header_1.protocol_data().clone(),
    )
    .is_ok());
    assert!(client::assert_encoding_for_protocol_data(
        protocol_hash_1.clone(),
        block_header_2.protocol_data().clone(),
    )
    .is_err());
    assert!(client::assert_encoding_for_protocol_data(
        protocol_hash_2.clone(),
        block_header_1.protocol_data().clone(),
    )
    .is_err());
    assert!(client::assert_encoding_for_protocol_data(
        protocol_hash_2.clone(),
        block_header_2.protocol_data().clone(),
    )
    .is_ok());
}

/// Initializes empty dir for ocaml storage
fn prepare_protocol_context(
    dir_name: &str,
    tezos_env: &TezosEnvironment,
    commit_genesis: bool,
) -> InitProtocolContextResult {
    let cfg = environment::TEZOS_ENV
        .get(tezos_env)
        .expect("no tezos environment configured");

    // init empty storage for test
    let storage_data_dir_path = common::prepare_empty_dir(dir_name);
    let storage_init_info = ffi::init_protocol_context(
        storage_data_dir_path.to_string(),
        cfg.genesis.clone(),
        cfg.protocol_overrides.clone(),
        commit_genesis,
        false,
        false,
        None,
    )
    .unwrap();

    storage_init_info
}
