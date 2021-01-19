// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Tests for apply first blocks for protocol V0 based

use serial_test::serial;

use crypto::hash::{ChainId, ProtocolHash};
use tezos_api::environment::{
    TezosEnvironmentConfiguration, OPERATION_LIST_LIST_HASH_EMPTY, TEZOS_ENV,
};
use tezos_api::ffi::{
    ApplyBlockError, ApplyBlockRequest, BeginApplicationRequest, InitProtocolContextResult,
    TezosRuntimeConfiguration,
};
use tezos_client::client;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

mod common;

fn init_test_runtime() {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        log_enabled: common::is_ocaml_log_enabled(),
        no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
    })
    .unwrap();
}

fn init_test_protocol_context(
    dir_name: &str,
) -> (
    ChainId,
    BlockHeader,
    ProtocolHash,
    InitProtocolContextResult,
) {
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
        .get(&test_data::TEZOS_NETWORK)
        .expect("no tezos environment configured");

    let result = client::init_protocol_context(
        common::prepare_empty_dir(dir_name),
        tezos_env.genesis.clone(),
        tezos_env.protocol_overrides.clone(),
        true,
        false,
        false,
        None,
    )
    .unwrap();

    let genesis_commit_hash = match result.clone().genesis_commit_hash {
        None => panic!("we needed commit_genesis and here should be result of it"),
        Some(cr) => cr,
    };

    (
        tezos_env.main_chain_id().expect("invalid chain id"),
        tezos_env
            .genesis_header(genesis_commit_hash, OPERATION_LIST_LIST_HASH_EMPTY.clone())
            .expect("genesis header error"),
        tezos_env.genesis_protocol().expect("protocol_hash error"),
        result,
    )
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_first_three_blocks() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) =
        init_test_protocol_context("bootstrap_test_storage_01");

    // apply first block - level 1
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap(),
        )
        .unwrap(),
        pred_header: genesis_block_header,
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    })
    .unwrap();
    assert_eq!(
        test_data::context_hash(test_data::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH),
        apply_block_result.context_hash
    );
    assert_eq!(1, apply_block_result.max_operations_ttl);
    assert!(apply_block_result.block_metadata_hash.is_none());
    assert!(apply_block_result.ops_metadata_hash.is_none());
    assert!(apply_block_result.ops_metadata_hashes.is_none());

    // apply second block - level 2
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_2,
            test_data::block_header_level2_operations(),
        )),
        max_operations_ttl: apply_block_result.max_operations_ttl,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    })
    .unwrap();
    assert_eq!(
        "lvl 2, fit 2, prio 5, 0 ops",
        &apply_block_result.validation_result_message
    );
    assert_eq!(2, apply_block_result.max_operations_ttl);

    // apply third block - level 3
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id,
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_3).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_3,
            test_data::block_header_level3_operations(),
        )),
        max_operations_ttl: apply_block_result.max_operations_ttl,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    })
    .unwrap();
    assert_eq!(
        "lvl 3, fit 5, prio 12, 1 ops",
        &apply_block_result.validation_result_message
    );
    assert_eq!(3, apply_block_result.max_operations_ttl);
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_first_block_twice() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) =
        init_test_protocol_context("bootstrap_test_storage_09");

    // apply first block - level 0
    let apply_block_result_1 = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap(),
        )
        .unwrap(),
        pred_header: genesis_block_header.clone(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    let apply_block_result_1 = apply_block_result_1.unwrap();
    assert_eq!(
        test_data::context_hash(test_data::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH),
        apply_block_result_1.context_hash
    );

    // apply first block second time - level 0
    let apply_block_result_2 = client::apply_block(ApplyBlockRequest {
        chain_id,
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap(),
        )
        .unwrap(),
        pred_header: genesis_block_header,
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    let apply_block_result_2 = apply_block_result_2.unwrap();
    assert_eq!(
        test_data::context_hash(test_data::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH),
        apply_block_result_2.context_hash
    );

    // results should be eq
    assert_eq!(apply_block_result_1, apply_block_result_2);
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_first_two_blocks_and_check_result_json_metadata() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, genesis_protocol_hash, result) =
        init_test_protocol_context("bootstrap_test_storage_10");

    // check genesis data
    let genesis_context_hash = result.genesis_commit_hash.expect("no genesis context_hash");
    let genesis_data =
        client::genesis_result_data(&genesis_context_hash, &chain_id, &genesis_protocol_hash, 0)
            .expect("no genesis data");
    assert_contains_metadata(
        &genesis_data.block_header_proto_metadata_json,
        vec![
            "protocol",
            "next_protocol",
            "test_chain_status",
            "max_operations_ttl",
            "max_operation_data_length",
            "max_block_header_length",
            "max_operation_list_length",
        ],
    );

    // apply first block - level 0
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap(),
        )
        .unwrap(),
        pred_header: genesis_block_header,
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    })
    .unwrap();

    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["content", "signature"],
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_metadata_json,
        vec![
            "protocol",
            "next_protocol",
            "test_chain_status",
            "max_operations_ttl",
            "max_operation_data_length",
            "max_block_header_length",
            "max_operation_list_length",
        ],
    );

    // apply second block - level 2
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_2,
            test_data::block_header_level2_operations(),
        )),
        max_operations_ttl: 1,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    })
    .unwrap();

    assert_eq!(
        "lvl 2, fit 2, prio 5, 0 ops",
        &apply_block_result.validation_result_message
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["signature", "proof_of_work_nonce", "priority"],
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_metadata_json,
        vec![
            "protocol",
            "next_protocol",
            "test_chain_status",
            "max_operations_ttl",
            "max_operation_data_length",
            "max_block_header_length",
            "max_operation_list_length",
            "baker",
            "level",
            "balance_updates",
        ],
    );

    // apply the second block twice, should return the same data
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_2,
            test_data::block_header_level2_operations(),
        )),
        max_operations_ttl: 1,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    })
    .unwrap();

    assert_eq!(
        "lvl 2, fit 2, prio 5, 0 ops",
        &apply_block_result.validation_result_message
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["signature", "proof_of_work_nonce", "priority"],
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_metadata_json,
        vec![
            "protocol",
            "next_protocol",
            "test_chain_status",
            "max_operations_ttl",
            "max_operation_data_length",
            "max_block_header_length",
            "max_operation_list_length",
            "baker",
            "level",
            "balance_updates",
        ],
    );

    // apply third block - level 3
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id,
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_3).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_3,
            test_data::block_header_level3_operations(),
        )),
        max_operations_ttl: 2,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    })
    .unwrap();
    assert_eq!(
        "lvl 3, fit 5, prio 12, 1 ops",
        &apply_block_result.validation_result_message
    );

    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["signature", "proof_of_work_nonce", "priority"],
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_metadata_json,
        vec![
            "protocol",
            "next_protocol",
            "test_chain_status",
            "max_operations_ttl",
            "max_operation_data_length",
            "max_block_header_length",
            "max_operation_list_length",
            "baker",
            "level",
            "balance_updates",
        ],
    );
    assert_contains_metadata(
        &apply_block_result.operations_proto_metadata_json,
        vec!["protocol", "contents", "balance_updates"],
    );
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_second_block_with_first_predecessor_should_fail_unknown_predecessor_context(
) {
    init_test_runtime();

    // init empty context for test
    let (chain_id, ..) = init_test_protocol_context("bootstrap_test_storage_02");

    // apply second block - level 2
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_2,
            test_data::block_header_level2_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    assert!(apply_block_result.is_err());
    assert!(match apply_block_result.unwrap_err() {
        ApplyBlockError::UnknownPredecessorContext { .. } => true,
        e => {
            println!("expected UnknownPredecessorContext, but was {:?}", e);
            false
        }
    });
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_third_block_with_first_predecessor_should_fail_predecessor_mismatch(
) {
    init_test_runtime();

    // init empty context for test
    let (chain_id, ..) = init_test_protocol_context("bootstrap_test_storage_18");

    // apply second block - level 2
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id,
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_3).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_3,
            test_data::block_header_level3_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    assert!(apply_block_result.is_err());
    assert!(match apply_block_result.unwrap_err() {
        ApplyBlockError::PredecessorMismatch { .. } => true,
        e => {
            println!("expected PredecessorMismatch, but was {:?}", e);
            false
        }
    });
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_second_block_should_fail_incomplete_operations() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) =
        init_test_protocol_context("bootstrap_test_storage_03");

    // apply second block - level 3 has validation_pass = 4
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id,
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_3).unwrap(),
        )
        .unwrap(),
        pred_header: genesis_block_header,
        operations: vec![vec![]],
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    assert!(apply_block_result.is_err());
    assert_eq!(
        ApplyBlockError::IncompleteOperations {
            expected: 4,
            actual: 1
        },
        apply_block_result.unwrap_err()
    );
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_first_block_with_invalid_operations_should_fail_invalid_operations(
) {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) =
        init_test_protocol_context("bootstrap_test_storage_04");

    // apply second block - level 1 ok
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap(),
        )
        .unwrap(),
        pred_header: genesis_block_header,
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    assert!(apply_block_result.is_ok());

    // apply second block - level 2 with operations for level 3
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id,
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap(),
        )
        .unwrap(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())
            .unwrap(),
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_3,
            test_data::block_header_level3_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    assert!(apply_block_result.is_err());
}

#[test]
#[serial]
fn test_begin_application_on_empty_storage_with_first_blocks() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) =
        init_test_protocol_context("test_begin_application_on_empty_storage_with_first_block");

    // begin application for first block - level 1
    let result = client::begin_application(BeginApplicationRequest {
        chain_id: chain_id.clone(),
        pred_header: genesis_block_header.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap(),
        )
        .unwrap(),
    });
    assert!(result.is_ok());

    // begin application for second block - level 2 - should fail (because genesis has different protocol)
    let result = client::begin_application(BeginApplicationRequest {
        chain_id: chain_id.clone(),
        pred_header: genesis_block_header.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap(),
        )
        .unwrap(),
    });
    assert!(result.is_err());

    // apply second block - level 1 ok
    let apply_block_result = client::apply_block(ApplyBlockRequest {
        chain_id: chain_id.clone(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap(),
        )
        .unwrap(),
        pred_header: genesis_block_header,
        operations: ApplyBlockRequest::convert_operations(test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        )),
        max_operations_ttl: 0,
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    });
    assert!(apply_block_result.is_ok());

    // begin application for second block - level 2 - now it should work on first level
    let result = client::begin_application(BeginApplicationRequest {
        chain_id: chain_id.clone(),
        pred_header: BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap())
            .unwrap(),
        block_header: BlockHeader::from_bytes(
            hex::decode(test_data::BLOCK_HEADER_LEVEL_2).unwrap(),
        )
        .unwrap(),
    });
    assert!(result.is_ok());
}

fn assert_contains_metadata(metadata: &str, expected_attributes: Vec<&str>) {
    expected_attributes
        .iter()
        .for_each(|expected_attribute| assert_contains(&metadata, expected_attribute));
}

fn assert_contains(value: &str, attribute: &str) {
    if !value.contains(attribute) {
        panic!(
            "assert_contains failed: value: `{:?}` does not contains: `{:?}`",
            &value, attribute
        );
    }
}

mod test_data {
    use crypto::hash::{ContextHash, HashType};
    use tezos_api::environment::TezosEnvironment;
    use tezos_messages::p2p::binary_message::BinaryMessage;
    use tezos_messages::p2p::encoding::prelude::*;

    pub const TEZOS_NETWORK: TezosEnvironment = TezosEnvironment::Alphanet;

    pub fn context_hash(hash: &str) -> ContextHash {
        HashType::ContextHash.b58check_to_hash(hash).unwrap()
    }

    // BMPtRJqFGQJRTfn8bXQR2grLE1M97XnUmG5vgjHMW7St1Wub7Cd
    pub const BLOCK_HEADER_HASH_LEVEL_1: &str =
        "dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d";
    pub const BLOCK_HEADER_LEVEL_1: &str = include_str!("resources/block_header_level1.bytes");
    pub const BLOCK_HEADER_LEVEL_1_CONTEXT_HASH: &str =
        "CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE";

    pub fn block_header_level1_operations() -> Vec<Vec<String>> {
        vec![]
    }

    // BLwKksYwrxt39exDei7yi47h7aMcVY2kZMZhTwEEoSUwToQUiDV
    pub const BLOCK_HEADER_HASH_LEVEL_2: &str =
        "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627";
    pub const BLOCK_HEADER_LEVEL_2: &str = "0000000201dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d000000005c017cd804683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d0000001100000001000000000800000000000000029bd8c75dec93c276d2d8e8febc3aa6c9471cb2cb42236b3ab4ca5f1f2a0892f6000500000003ba671eef00d6a8bea20a4677fae51268ab6be7bd8cfc373cd6ac9e0a00064efcc404e1fb39409c5df255f7651e3d1bb5d91cb2172b687e5d56ebde58cfd92e1855aaafbf05";

    pub fn block_header_level2_operations() -> Vec<Vec<String>> {
        vec![vec![], vec![], vec![], vec![]]
    }

    // BLTQ5B4T4Tyzqfm3Yfwi26WmdQScr6UXVSE9du6N71LYjgSwbtc
    pub const BLOCK_HEADER_HASH_LEVEL_3: &str =
        "61e687e852460b28f0f9540ccecf8f6cf87a5ad472c814612f0179caf4b9f673";
    pub const BLOCK_HEADER_LEVEL_3: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";

    pub fn block_header_level3_operations() -> Vec<Vec<String>> {
        vec![
            vec!["a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a".to_string()],
            vec![],
            vec![],
            vec![]
        ]
    }

    pub fn block_operations_from_hex(
        block_hash: &str,
        hex_operations: Vec<Vec<String>>,
    ) -> Vec<OperationsForBlocksMessage> {
        hex_operations
            .into_iter()
            .map(|bo| {
                let ops = bo
                    .into_iter()
                    .map(|op| Operation::from_bytes(hex::decode(op).unwrap()).unwrap())
                    .collect();
                OperationsForBlocksMessage::new(
                    OperationsForBlock::new(hex::decode(block_hash).unwrap(), 4),
                    Path::Op,
                    ops,
                )
            })
            .collect()
    }
}
