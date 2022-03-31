// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryFrom;

use crypto::hash::ProtocolHash;
use serial_test::serial;
use tezos_api::environment::{self, TezosEnvironment};
use tezos_api::ffi::{
    InitProtocolContextResult, ProtocolDataError, RustBytes, TezosRuntimeConfiguration,
    TezosRuntimeLogLevel,
};
use tezos_context_api::{
    TezosContextIrminStorageConfiguration, TezosContextStorageConfiguration,
    TezosContextTezEdgeStorageConfiguration, TezosContextTezedgeOnDiskBackendOptions,
};
use tezos_interop::apply_encoded_message;
use tezos_messages::p2p::binary_message::BinaryRead;
use tezos_messages::p2p::encoding::prelude::BlockHeader;
use tezos_protocol_ipc_messages::{InitProtocolContextParams, NodeMessage, ProtocolMessage};

mod common;

fn assert_encoding_for_protocol_data(
    protocol_hash: ProtocolHash,
    protocol_data: RustBytes,
) -> Result<(), ProtocolDataError> {
    let result = apply_encoded_message(ProtocolMessage::AssertEncodingForProtocolDataCall(
        protocol_hash,
        protocol_data,
    ))
    .unwrap();
    expect_response!(AssertEncodingForProtocolDataResult, result)
}

#[test]
#[serial]
fn test_init_protocol_context() {
    // change cfg
    apply_encoded_message(ProtocolMessage::ChangeRuntimeConfigurationCall(
        TezosRuntimeConfiguration {
            log_level: Some(TezosRuntimeLogLevel::Info),
            log_enabled: common::is_ocaml_log_enabled(),
        },
    ))
    .unwrap();

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
        genesis_commit_hash.to_base58_check(),
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

    let protocol_hash_1 =
        ProtocolHash::try_from("PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex")
            .expect("Failed to decode protocol hash");
    let protocol_hash_2 =
        ProtocolHash::try_from("PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS")
            .expect("Failed to decode protocol hash");

    // check
    assert!(assert_encoding_for_protocol_data(
        protocol_hash_1.clone(),
        block_header_1.protocol_data().clone().into(),
    )
    .is_ok());
    assert!(assert_encoding_for_protocol_data(
        protocol_hash_1,
        block_header_2.protocol_data().clone().into(),
    )
    .is_err());
    assert!(assert_encoding_for_protocol_data(
        protocol_hash_2.clone(),
        block_header_1.protocol_data().clone().into(),
    )
    .is_err());
    assert!(assert_encoding_for_protocol_data(
        protocol_hash_2,
        block_header_2.protocol_data().clone().into(),
    )
    .is_ok());
}

/// Initializes empty dir for ocaml storage
fn prepare_protocol_context(
    dir_name: &str,
    tezos_env: &TezosEnvironment,
    commit_genesis: bool,
) -> InitProtocolContextResult {
    let default_networks = environment::default_networks();
    let cfg = default_networks
        .get(tezos_env)
        .expect("no tezos environment configured");

    // init empty storage for test
    let storage = TezosContextStorageConfiguration::Both(
        TezosContextIrminStorageConfiguration {
            data_dir: common::prepare_empty_dir(dir_name),
        },
        TezosContextTezEdgeStorageConfiguration {
            backend: tezos_context_api::ContextKvStoreConfiguration::InMem(
                TezosContextTezedgeOnDiskBackendOptions {
                    base_path: dir_name.to_string(),
                    startup_check: false,
                },
            ),
            ipc_socket_path: None,
        },
    );
    let context_config = InitProtocolContextParams {
        storage,
        genesis: cfg.genesis.clone(),
        protocol_overrides: cfg.protocol_overrides.clone(),
        commit_genesis,
        enable_testchain: false,
        readonly: false,
        patch_context: None,
        context_stats_db_path: None,
        genesis_max_operations_ttl: cfg.genesis_additional_data().unwrap().max_operations_ttl,
    };

    let result =
        apply_encoded_message(ProtocolMessage::InitProtocolContextCall(context_config)).unwrap();
    expect_response!(InitProtocolContextResult, result).unwrap()
}
