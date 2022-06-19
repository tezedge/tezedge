// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Tests for apply first blocks for protocol V1 based like 008 edo

use serial_test::serial;

use crypto::hash::{BlockMetadataHash, OperationMetadataHash, OperationMetadataListListHash};

use tezos_api::ffi::{ApplyBlockError, ApplyBlockRequest, BeginApplicationRequest};

use tezos_interop::apply_encoded_message;
use tezos_messages::p2p::binary_message::BinaryRead;
use tezos_messages::p2p::encoding::prelude::*;
use tezos_protocol_ipc_messages::{
    GenesisResultDataParams, JsonEncodeApplyBlockOperationsMetadataParams,
    JsonEncodeApplyBlockResultMetadataParams, NodeMessage, ProtocolMessage,
};

use crate::common::{init_test_protocol_context, init_test_runtime};

mod common;

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_first_four_blocks_protocol_v1() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) = init_test_protocol_context(
        "bootstrap_test_storage_11",
        test_data_protocol_v1::tezos_network(),
    );

    // apply first block - level 1
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header,
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_1,
                    test_data_protocol_v1::block_header_level1_operations(),
                ),
            ),
            max_operations_ttl: 0,
            // Note: we dont have this information from genesis block / commit_genesis, so we send None for both
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();
    assert_eq!(
        test_data_protocol_v1::context_hash(
            test_data_protocol_v1::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH
        ),
        apply_block_result.context_hash
    );
    assert_eq!(1, apply_block_result.max_operations_ttl);
    assert_block_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_1_BLOCK_METADATA_HASH,
        &apply_block_result.block_metadata_hash,
    );
    assert_operation_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_1_OPERATION_METADATA_LIST_LIST_HASH,
        &apply_block_result.ops_metadata_hash,
    );
    assert_operation_metadata_hashes(
        test_data_protocol_v1::block_header_level1_operation_metadata_hashes(),
        &apply_block_result.ops_metadata_hashes,
    );

    // apply second block - level 2
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_2,
                    test_data_protocol_v1::block_header_level2_operations(),
                ),
            ),
            max_operations_ttl: apply_block_result.max_operations_ttl,
            predecessor_block_metadata_hash: apply_block_result.block_metadata_hash,
            // Note: we know that there is a fix in ocaml, where they do handle predecessor_ops_metadata_hash, if operations are present, means [`if validation_passes > 0`] for predecessor
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();

    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();

    assert_eq!(
        test_data_protocol_v1::context_hash(
            test_data_protocol_v1::BLOCK_HEADER_LEVEL_2_CONTEXT_HASH
        ),
        apply_block_result.context_hash
    );
    assert_eq!(2, apply_block_result.max_operations_ttl);
    assert_eq!(
        "lvl 2, fit 1:1, prio 8, 0 ops",
        &apply_block_result.validation_result_message
    );
    assert_block_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_2_BLOCK_METADATA_HASH,
        &apply_block_result.block_metadata_hash,
    );
    assert_operation_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_2_OPERATION_METADATA_LIST_LIST_HASH,
        &apply_block_result.ops_metadata_hash,
    );
    assert_operation_metadata_hashes(
        test_data_protocol_v1::block_header_level2_operation_metadata_hashes(),
        &apply_block_result.ops_metadata_hashes,
    );

    // apply third block - level 3
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_3).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_3,
                    test_data_protocol_v1::block_header_level3_operations(),
                ),
            ),
            max_operations_ttl: apply_block_result.max_operations_ttl,
            predecessor_block_metadata_hash: apply_block_result.block_metadata_hash,
            predecessor_ops_metadata_hash: apply_block_result.ops_metadata_hash,
        }))
        .unwrap();

    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();

    assert_eq!(
        test_data_protocol_v1::context_hash(
            test_data_protocol_v1::BLOCK_HEADER_LEVEL_3_CONTEXT_HASH
        ),
        apply_block_result.context_hash
    );
    assert_eq!(3, apply_block_result.max_operations_ttl);
    assert_eq!(
        "lvl 3, fit 1:2, prio 2, 1 ops",
        &apply_block_result.validation_result_message
    );
    assert_block_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_3_BLOCK_METADATA_HASH,
        &apply_block_result.block_metadata_hash,
    );
    assert_operation_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_3_OPERATION_METADATA_LIST_LIST_HASH,
        &apply_block_result.ops_metadata_hash,
    );
    assert_operation_metadata_hashes(
        test_data_protocol_v1::block_header_level3_operation_metadata_hashes(),
        &apply_block_result.ops_metadata_hashes,
    );

    // apply third block - level 4
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id,
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_4).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_3).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_4,
                    test_data_protocol_v1::block_header_level4_operations(),
                ),
            ),
            max_operations_ttl: apply_block_result.max_operations_ttl,
            predecessor_block_metadata_hash: apply_block_result.block_metadata_hash,
            predecessor_ops_metadata_hash: apply_block_result.ops_metadata_hash,
        }))
        .unwrap();

    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();

    assert_eq!(
        test_data_protocol_v1::context_hash(
            test_data_protocol_v1::BLOCK_HEADER_LEVEL_4_CONTEXT_HASH
        ),
        apply_block_result.context_hash
    );
    assert_eq!(4, apply_block_result.max_operations_ttl);
    assert_eq!(
        "lvl 4, fit 1:3, prio 1, 3 ops",
        &apply_block_result.validation_result_message
    );
    assert_block_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_4_BLOCK_METADATA_HASH,
        &apply_block_result.block_metadata_hash,
    );
    assert_operation_metadata_hash(
        test_data_protocol_v1::BLOCK_HEADER_LEVEL_4_OPERATION_METADATA_LIST_LIST_HASH,
        &apply_block_result.ops_metadata_hash,
    );
    assert_operation_metadata_hashes(
        test_data_protocol_v1::block_header_level4_operation_metadata_hashes(),
        &apply_block_result.ops_metadata_hashes,
    );
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_first_block_twice() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) = init_test_protocol_context(
        "bootstrap_test_storage_09",
        test_data_protocol_v1::tezos_network(),
    );

    // apply first block - level 0
    let apply_block_result_1 =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header.clone(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_1,
                    test_data_protocol_v1::block_header_level1_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result_1 = expect_response!(ApplyBlockResult, apply_block_result_1).unwrap();
    assert_eq!(
        test_data_protocol_v1::context_hash(
            test_data_protocol_v1::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH
        ),
        apply_block_result_1.context_hash
    );

    // apply first block second time - level 0
    let apply_block_result_2 =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id,
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header,
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_1,
                    test_data_protocol_v1::block_header_level1_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let mut apply_block_result_2 =
        expect_response!(ApplyBlockResult, apply_block_result_2).unwrap();
    assert_eq!(
        test_data_protocol_v1::context_hash(
            test_data_protocol_v1::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH
        ),
        apply_block_result_2.context_hash
    );

    // Commit time will differ, make it be the same so that the assert
    // doesn't fail because of this.
    apply_block_result_2.commit_time = apply_block_result_1.commit_time;
    apply_block_result_2.execution_timestamps = apply_block_result_1.execution_timestamps.clone();

    // results should be eq
    assert_eq!(apply_block_result_1, apply_block_result_2);
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_first_two_blocks_and_check_result_json_metadata() {
    // init empty context for test
    let (chain_id, genesis_block_header, genesis_additional_data, result) =
        init_test_protocol_context(
            "bootstrap_test_storage_10",
            test_data_protocol_v1::tezos_network(),
        );

    // check genesis data
    let genesis_context_hash = result.genesis_commit_hash.expect("no genesis context_hash");
    let genesis_data = apply_encoded_message(ProtocolMessage::GenesisResultDataCall(
        GenesisResultDataParams {
            genesis_context_hash: genesis_context_hash.clone(),
            chain_id: chain_id.clone(),
            genesis_protocol_hash: genesis_additional_data.next_protocol_hash.clone(),
            genesis_max_operations_ttl: genesis_additional_data.max_operations_ttl,
        },
    ))
    .unwrap();
    let genesis_data =
        expect_response!(CommitGenesisResultData, genesis_data).expect("no genesis data");

    let block_header_proto_metadata_json =
        apply_encoded_message(ProtocolMessage::JsonEncodeApplyBlockResultMetadata(
            JsonEncodeApplyBlockResultMetadataParams {
                context_hash: genesis_context_hash.clone(),
                metadata_bytes: genesis_data.block_header_proto_metadata_bytes.clone(),
                max_operations_ttl: 0,
                protocol_hash: genesis_additional_data.protocol_hash.clone(),
                next_protocol_hash: genesis_additional_data.next_protocol_hash.clone(),
            },
        ))
        .unwrap();
    let block_header_proto_metadata_json = expect_response!(
        JsonEncodeApplyBlockResultMetadataResponse,
        block_header_proto_metadata_json
    )
    .expect("failed to get genesis json");
    assert_contains_metadata(
        &block_header_proto_metadata_json,
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

    let operations_proto_metadata_json =
        apply_encoded_message(ProtocolMessage::JsonEncodeApplyBlockOperationsMetadata(
            JsonEncodeApplyBlockOperationsMetadataParams {
                chain_id: chain_id.clone(),
                operations: vec![],
                operations_metadata_bytes: genesis_data.operations_proto_metadata_bytes,
                protocol_hash: genesis_additional_data.protocol_hash.clone(),
                next_protocol_hash: genesis_additional_data.next_protocol_hash.clone(),
            },
        ))
        .unwrap();
    let operations_proto_metadata_json = expect_response!(
        JsonEncodeApplyBlockOperationsMetadata,
        operations_proto_metadata_json
    )
    .expect("failed to get genesis json");
    assert_eq!("[]", operations_proto_metadata_json);

    let max_operations_ttl = genesis_additional_data.max_operations_ttl.into();

    // apply first block - level 0
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header,
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_1,
                    test_data_protocol_v1::block_header_level1_operations(),
                ),
            ),
            max_operations_ttl,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();

    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["content", "signature"],
    );
    let apply_block_result_metadata_json =
        apply_encoded_message(ProtocolMessage::JsonEncodeApplyBlockResultMetadata(
            JsonEncodeApplyBlockResultMetadataParams {
                context_hash: genesis_context_hash.clone(),
                metadata_bytes: apply_block_result.block_header_proto_metadata_bytes,
                max_operations_ttl,
                protocol_hash: apply_block_result.protocol_hash,
                next_protocol_hash: apply_block_result.next_protocol_hash,
            },
        ))
        .unwrap();
    let apply_block_result_metadata_json = expect_response!(
        JsonEncodeApplyBlockResultMetadataResponse,
        apply_block_result_metadata_json
    )
    .unwrap();

    assert_contains_metadata(
        &apply_block_result_metadata_json,
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

    let max_operations_ttl = 1;

    let predecessor_block_metadata_hash = apply_block_result.block_metadata_hash;

    // apply second block - level 2
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_2,
                    test_data_protocol_v1::block_header_level2_operations(),
                ),
            ),
            max_operations_ttl,
            predecessor_block_metadata_hash: predecessor_block_metadata_hash.clone(),
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();

    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();

    assert_eq!(
        "lvl 2, fit 1:1, prio 8, 0 ops",
        &apply_block_result.validation_result_message
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["signature", "proof_of_work_nonce", "priority"],
    );

    let apply_block_result_metadata_json =
        apply_encoded_message(ProtocolMessage::JsonEncodeApplyBlockResultMetadata(
            JsonEncodeApplyBlockResultMetadataParams {
                context_hash: genesis_context_hash.clone(),
                metadata_bytes: apply_block_result.block_header_proto_metadata_bytes,
                max_operations_ttl,
                protocol_hash: apply_block_result.protocol_hash,
                next_protocol_hash: apply_block_result.next_protocol_hash,
            },
        ))
        .unwrap();
    let apply_block_result_metadata_json = expect_response!(
        JsonEncodeApplyBlockResultMetadataResponse,
        apply_block_result_metadata_json
    )
    .unwrap();

    assert_contains_metadata(
        &apply_block_result_metadata_json,
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

    let max_operations_ttl = 1;

    // apply the second block twice, should return the same data
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_2,
                    test_data_protocol_v1::block_header_level2_operations(),
                ),
            ),
            max_operations_ttl,
            predecessor_block_metadata_hash,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();

    assert_eq!(
        "lvl 2, fit 1:1, prio 8, 0 ops",
        &apply_block_result.validation_result_message
    );
    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["signature", "proof_of_work_nonce", "priority"],
    );

    let apply_block_result_metadata_json =
        apply_encoded_message(ProtocolMessage::JsonEncodeApplyBlockResultMetadata(
            JsonEncodeApplyBlockResultMetadataParams {
                context_hash: genesis_context_hash.clone(),
                metadata_bytes: apply_block_result.block_header_proto_metadata_bytes,
                max_operations_ttl,
                protocol_hash: apply_block_result.protocol_hash,
                next_protocol_hash: apply_block_result.next_protocol_hash,
            },
        ))
        .unwrap();
    let apply_block_result_metadata_json = expect_response!(
        JsonEncodeApplyBlockResultMetadataResponse,
        apply_block_result_metadata_json
    )
    .unwrap();

    assert_contains_metadata(
        &apply_block_result_metadata_json,
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

    let operations =
        ApplyBlockRequest::convert_operations(test_data_protocol_v1::block_operations_from_hex(
            test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_3,
            test_data_protocol_v1::block_header_level3_operations(),
        ));
    let max_operations_ttl = 2;

    // apply third block - level 3
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_3).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            operations: operations.clone(),
            max_operations_ttl,
            predecessor_block_metadata_hash: apply_block_result.block_metadata_hash,
            predecessor_ops_metadata_hash: apply_block_result.ops_metadata_hash,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();

    assert_eq!(
        "lvl 3, fit 1:2, prio 2, 1 ops",
        &apply_block_result.validation_result_message
    );

    assert_contains_metadata(
        &apply_block_result.block_header_proto_json,
        vec!["signature", "proof_of_work_nonce", "priority"],
    );

    let apply_block_result_metadata_json =
        apply_encoded_message(ProtocolMessage::JsonEncodeApplyBlockResultMetadata(
            JsonEncodeApplyBlockResultMetadataParams {
                context_hash: genesis_context_hash,
                metadata_bytes: apply_block_result.block_header_proto_metadata_bytes.clone(),
                max_operations_ttl,
                protocol_hash: apply_block_result.protocol_hash.clone(),
                next_protocol_hash: apply_block_result.next_protocol_hash.clone(),
            },
        ))
        .unwrap();
    let apply_block_result_metadata_json = expect_response!(
        JsonEncodeApplyBlockResultMetadataResponse,
        apply_block_result_metadata_json
    )
    .unwrap();

    assert_contains_metadata(
        &apply_block_result_metadata_json,
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

    let apply_block_operations_metadata_json =
        apply_encoded_message(ProtocolMessage::JsonEncodeApplyBlockOperationsMetadata(
            JsonEncodeApplyBlockOperationsMetadataParams {
                chain_id,
                operations,
                operations_metadata_bytes: apply_block_result
                    .operations_proto_metadata_bytes
                    .clone(),
                protocol_hash: apply_block_result.protocol_hash.clone(),
                next_protocol_hash: apply_block_result.next_protocol_hash,
            },
        ))
        .unwrap();
    let apply_block_operations_metadata_json = expect_response!(
        JsonEncodeApplyBlockOperationsMetadata,
        apply_block_operations_metadata_json
    )
    .unwrap();

    assert_contains_metadata(
        &apply_block_operations_metadata_json,
        vec!["protocol", "contents", "balance_updates"],
    );
}

#[test]
#[serial]
fn test_bootstrap_empty_storage_with_second_block_with_first_predecessor_should_fail_unknown_predecessor_context(
) {
    init_test_runtime();

    // init empty context for test
    let (chain_id, ..) = init_test_protocol_context(
        "bootstrap_test_storage_02",
        test_data_protocol_v1::tezos_network(),
    );

    // apply second block - level 2
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id,
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_2,
                    test_data_protocol_v1::block_header_level2_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result);
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
    let (chain_id, ..) = init_test_protocol_context(
        "bootstrap_test_storage_18",
        test_data_protocol_v1::tezos_network(),
    );

    // apply second block - level 2
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id,
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_3).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_3,
                    test_data_protocol_v1::block_header_level3_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result);
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
    let (chain_id, genesis_block_header, ..) = init_test_protocol_context(
        "bootstrap_test_storage_03",
        test_data_protocol_v1::tezos_network(),
    );

    // apply second block - level 3 has validation_pass = 4
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id,
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_3).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header,
            operations: vec![vec![]],
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result);
    assert!(apply_block_result.is_err());

    assert_eq!(
        ApplyBlockError::IncompleteOperations {
            expected: 4,
            actual: 1,
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
    let (chain_id, genesis_block_header, ..) = init_test_protocol_context(
        "bootstrap_test_storage_04",
        test_data_protocol_v1::tezos_network(),
    );

    // apply second block - level 1 ok
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header,
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_1,
                    test_data_protocol_v1::block_header_level1_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result);
    assert!(apply_block_result.is_ok());

    // apply second block - level 2 with operations for level 3
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id,
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_3,
                    test_data_protocol_v1::block_header_level3_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result);
    assert!(apply_block_result.is_err());
}

#[test]
#[serial]
fn test_begin_application_on_empty_storage_with_first_blocks() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) = init_test_protocol_context(
        "test_begin_application_on_empty_storage_with_first_block",
        test_data_protocol_v1::tezos_network(),
    );

    // begin application for first block - level 1
    let result = apply_encoded_message(ProtocolMessage::BeginApplicationCall(
        BeginApplicationRequest {
            chain_id: chain_id.clone(),
            pred_header: genesis_block_header.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
        },
    ))
    .unwrap();
    let result = expect_response!(BeginApplicationResult, result);
    assert!(result.is_ok());

    // begin application for second block - level 2 - should fail (because genesis has different protocol)
    let result = apply_encoded_message(ProtocolMessage::BeginApplicationCall(
        BeginApplicationRequest {
            chain_id: chain_id.clone(),
            pred_header: genesis_block_header.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
        },
    ))
    .unwrap();
    let result = expect_response!(BeginApplicationResult, result);
    assert!(result.is_err());

    // apply second block - level 1 ok
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header,
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v1::block_operations_from_hex(
                    test_data_protocol_v1::BLOCK_HEADER_HASH_LEVEL_1,
                    test_data_protocol_v1::block_header_level1_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result);
    assert!(apply_block_result.is_ok());

    // Ensure that we got the expected constants from the protocol change
    let apply_block_result = apply_block_result.unwrap();
    assert!(apply_block_result.new_protocol_constants_json.is_some());

    let expected_new_constants = serde_json::from_str::<serde_json::Value>(
        r#"{
            "proof_of_work_nonce_size": 8, "nonce_length": 32,
            "max_anon_ops_per_block": 132, "max_operation_data_length": 16384,
            "max_proposals_per_delegate": 20, "preserved_cycles": 3,
            "blocks_per_cycle": 2048, "blocks_per_commitment": 16,
            "blocks_per_roll_snapshot": 128, "blocks_per_voting_period": 2048,
            "time_between_blocks": [ "30", "20" ], "endorsers_per_block": 32,
            "hard_gas_limit_per_operation": "1040000",
            "hard_gas_limit_per_block": "10400000",
            "proof_of_work_threshold": "70368744177663",
            "tokens_per_roll": "8000000000", "michelson_maximum_type_size": 1000,
            "seed_nonce_revelation_tip": "125000", "origination_size": 257,
            "block_security_deposit": "512000000",
            "endorsement_security_deposit": "64000000",
            "baking_reward_per_endorsement": [ "1250000", "187500" ],
            "endorsement_reward": [ "1250000", "833333" ], "cost_per_byte": "250",
            "hard_storage_limit_per_operation": "60000",
            "test_chain_duration": "61440", "quorum_min": 2000, "quorum_max": 7000,
            "min_proposal_quorum": 500, "initial_endorsers": 24,
            "delay_per_missing_endorsement": "4"
        }"#,
    )
    .unwrap();
    let obtained_new_constants = serde_json::from_str::<serde_json::Value>(
        &apply_block_result.new_protocol_constants_json.unwrap(),
    )
    .unwrap();

    assert_eq!(expected_new_constants, obtained_new_constants);

    // Ensure that we got cycle roll owners snapshot data
    assert!(!apply_block_result.cycle_rolls_owner_snapshots.is_empty());
    assert!(!apply_block_result.cycle_rolls_owner_snapshots[0]
        .rolls_data
        .is_empty());

    // begin application for second block - level 2 - now it should work on first level
    let result = apply_encoded_message(ProtocolMessage::BeginApplicationCall(
        BeginApplicationRequest {
            chain_id,
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v1::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
        },
    ))
    .unwrap();
    let result = expect_response!(BeginApplicationResult, result);
    assert!(result.is_ok());
}

fn assert_contains_metadata(metadata: &str, expected_attributes: Vec<&str>) {
    expected_attributes
        .iter()
        .for_each(|expected_attribute| assert_contains(metadata, expected_attribute));
}

fn assert_contains(value: &str, attribute: &str) {
    if !value.contains(attribute) {
        panic!(
            "assert_contains failed: value: `{:?}` does not contains: `{:?}`",
            &value, attribute
        );
    }
}

fn assert_block_metadata_hash(expected_base58_string: &str, tested: &Option<BlockMetadataHash>) {
    match tested {
        Some(hash) => assert_eq!(expected_base58_string, &hash.to_base58_check()),
        None => panic!(
            "assert_block_metadata_hash failed: expecting : `{:?}` but has None",
            expected_base58_string
        ),
    }
}

fn assert_operation_metadata_hash(
    expected_base58_string: &str,
    tested: &Option<OperationMetadataListListHash>,
) {
    match tested {
        Some(hash) => assert_eq!(expected_base58_string, hash.to_base58_check()),
        None => panic!(
            "assert_operation_metadata_list_list_hash failed: expecting : `{:?}` but has None",
            expected_base58_string
        ),
    }
}

fn assert_operation_metadata_hashes(
    expected_base58_strings: Option<Vec<Vec<OperationMetadataHash>>>,
    tested: &Option<Vec<Vec<OperationMetadataHash>>>,
) {
    if expected_base58_strings.is_none() && tested.is_some() {
        panic!("assert_operation_metadata_hashes: Expected None, but has Some")
    }
    if expected_base58_strings.is_some() && tested.is_none() {
        panic!("assert_operation_metadata_hashes: Expected Some, but has None")
    }

    let expected_base58_strings = expected_base58_strings.unwrap_or_default();
    let tested = match tested.as_ref() {
        Some(hashes) => hashes.clone(),
        None => vec![],
    };

    assert_eq!(expected_base58_strings.len(), tested.len());
    expected_base58_strings
        .iter()
        .zip(tested)
        .for_each(|(expected, tested)| {
            assert_eq!(expected.len(), tested.len());
            expected.iter().zip(tested).for_each(|(e, t)| {
                assert_eq!(*e, t);
            })
        })
}

/// Test data for protocol_v1 like 008 edo
mod test_data_protocol_v1 {
    use std::convert::TryInto;

    use crypto::hash::{ContextHash, OperationMetadataHash};
    use tezos_api::environment::TezosEnvironmentConfiguration;
    use tezos_context_api::{GenesisChain, PatchContext, ProtocolOverrides};
    use tezos_messages::p2p::binary_message::BinaryRead;
    use tezos_messages::p2p::encoding::prelude::*;

    pub fn tezos_network() -> TezosEnvironmentConfiguration {
        TezosEnvironmentConfiguration {
            genesis: GenesisChain {
                time: "2020-11-30T12:00:00Z".to_string(),
                block: "BLockGenesisGenesisGenesisGenesisGenesis2431bbUwV2a".to_string(),
                protocol: "PtYuensgYBb3G3x1hLLbCmcav8ue8Kyd2khADcL5LsT5R1hcXex".to_string(),
            },
            bootstrap_lookup_addresses: vec![
                "51.75.246.56:9733".to_string(),
                "edonet.tezos.co.il".to_string(),
                "46.245.179.161:9733".to_string(),
                "edonet.smartpy.io".to_string(),
                "188.40.128.216:29732".to_string(),
                "51.79.165.131".to_string(),
                "edonet.boot.tezostaquito.io".to_string(),
                "95.216.228.228:9733".to_string(),
            ],
            version: "TEZOS_EDONET_2020-11-30T12:00:00Z".to_string(),
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: vec![],
                user_activated_protocol_overrides: vec![],
            },
            enable_testchain: true,
            patch_context_genesis_parameters: Some(PatchContext {
                key: "sandbox_parameter".to_string(),
                json: r#"{ "genesis_pubkey": "edpkugeDwmwuwyyD3Q5enapgEYDxZLtEUFFSrvVwXASQMVEqsvTqWu" }"#.to_string(),
            }),
        }
    }

    pub fn context_hash(hash: &str) -> ContextHash {
        ContextHash::from_base58_check(hash).unwrap()
    }

    // BLUzCt33hGwAsT4UdPXgqH2MjEZErpPfo5nL4rtQR5dStpixNrA
    pub const BLOCK_HEADER_HASH_LEVEL_1: &str =
        "6581e44bfff3b54e53e86b3587f7af579c12210a17f066f768aaf832c02c01c2";
    pub const BLOCK_HEADER_LEVEL_1: &str = include_str!("resources/edo_block_header_level1.bytes");
    pub const BLOCK_HEADER_LEVEL_1_CONTEXT_HASH: &str =
        "CoWT8jk3H1pHXz96JF7ke4SX3PVB9GZJCnDVf1Y5TSHvtNBsPaPT";
    pub const BLOCK_HEADER_LEVEL_1_BLOCK_METADATA_HASH: &str =
        "bm2gU1qwmoPNsXzFKydPDHWX37es6C5Z4nHyuesW8YxbkZ1339cN";
    pub const BLOCK_HEADER_LEVEL_1_OPERATION_METADATA_LIST_LIST_HASH: &str =
        "LLr1LNDRpBSmUrPMVJ5ViqSGg6tSXvxcQjfS5sayAVXdp28CnKuRv";

    pub fn block_header_level1_operations() -> Vec<Vec<String>> {
        vec![]
    }

    pub fn block_header_level1_operation_metadata_hashes() -> Option<Vec<Vec<OperationMetadataHash>>>
    {
        Some(vec![])
    }

    // BM5yiDGdmoubVFvnnU8DUtLPi48gsoqrwrJwt8yUkxa1NqZX5vt
    pub const BLOCK_HEADER_HASH_LEVEL_2: &str =
        "b4f5b6d362cdc21bebd5ec7fc829c5434ab316094a78ad05df93636ea7b1d3ee";
    pub const BLOCK_HEADER_LEVEL_2: &str = "00000002016581e44bfff3b54e53e86b3587f7af579c12210a17f066f768aaf832c02c01c2000000005fc4eaa404683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d000000110000000101000000080000000000000001fdb0f38fd96590f60cd5bad0b124c5db4c0c38ebad383eac852ca0eac27586120008c4907703fa490000009a05ac1d212b2b70b2d316e36cae41f63b0613483ae7e98cc12e7c8b45c6aaa52c68620caac9748d5116097756f7ae5bf217f095fb9750510d3b3045dbacd95d";
    pub const BLOCK_HEADER_LEVEL_2_CONTEXT_HASH: &str =
        "CoWa34JgZcJhQfSE9wugHiedhgW9vSaAscMTnfsF666oD26iGZKm";
    pub const BLOCK_HEADER_LEVEL_2_BLOCK_METADATA_HASH: &str =
        "bm3UWiFNLTbmWFus5zufLfJNScE1z8JDGJGPdSpceknyz4tV3K8u";
    pub const BLOCK_HEADER_LEVEL_2_OPERATION_METADATA_LIST_LIST_HASH: &str =
        "LLr21wqMDenTjZSDyHuCr7j1EZ5FJCdaMmwPjUjEFXdSwvYNrkBDk";

    pub fn block_header_level2_operations() -> Vec<Vec<String>> {
        vec![vec![], vec![], vec![], vec![]]
    }

    pub fn block_header_level2_operation_metadata_hashes() -> Option<Vec<Vec<OperationMetadataHash>>>
    {
        Some(vec![vec![], vec![], vec![], vec![]])
    }

    // BLDrZGd1XbCu4ed5Uf4MEW6bcbvghBw7mAuhxKvAEzUnR1EqDae
    pub const BLOCK_HEADER_HASH_LEVEL_3: &str =
        "43260c7a09634b82f3ba0f6d1f35b5fc3ea69d62aec9b11d4c07212f6c2da70f";
    pub const BLOCK_HEADER_LEVEL_3: &str = "0000000301b4f5b6d362cdc21bebd5ec7fc829c5434ab316094a78ad05df93636ea7b1d3ee000000005fc4eb3e0448ace2531441aea955e9f83f6a85c903adbf54f35ff05566bade10a65784dfcd0000001100000001010000000800000000000000028504db55af6d818e83c3fc2edcae47c95705d353a136ff9e4b52a427228be01d0002c490770315d20100001b9cf87118f406c592dfe2bed2fb63922f096283d2f40529468c81f4b0275147614c64e2ee833f60aab564978cd99be6be3ca21951e375dc447c04058383bddd";
    pub const BLOCK_HEADER_LEVEL_3_CONTEXT_HASH: &str =
        "CoVeteVf346zVuX8NqSJXZLsUPadEoyinyBKWkzUoV32Wzj5fuQN";
    pub const BLOCK_HEADER_LEVEL_3_BLOCK_METADATA_HASH: &str =
        "bm2nPre28WVr2B9serKB6XRZc8KMGSVq5Y8Vx7ZaptBVN8ZfvRWM";
    pub const BLOCK_HEADER_LEVEL_3_OPERATION_METADATA_LIST_LIST_HASH: &str =
        "LLr283rR7AWhepNeHcP9msa2VeAurWtodBLrnSjwaxpNyiyfhYcKX";

    pub fn block_header_level3_operations() -> Vec<Vec<String>> {
        vec![
            vec!["b4f5b6d362cdc21bebd5ec7fc829c5434ab316094a78ad05df93636ea7b1d3ee0000000002a521edcd56c091ebdeef3fde38c4d44e5cb100f0da703e567cdd07a667fedd312fb9a312261eb161ba8455e81f1ce70da1232a7ed4c2dc6583fb092aa0dd93c3".to_string()],
            vec![],
            vec![],
            vec![]
        ]
    }

    pub fn block_header_level3_operation_metadata_hashes() -> Option<Vec<Vec<OperationMetadataHash>>>
    {
        Some(vec![
            vec!["r3niv7sM81cVxAgKRy2NbYpj3JgAJfWGaqqeHZR63FqphTRpQqo"
                .try_into()
                .expect("Failed to decode hash")],
            vec![],
            vec![],
            vec![],
        ])
    }

    // BMMiqb1y5cqzdyNPC7iCCh4aLaVtzLNUMsaNfySHGCxTCVxd9Kz
    pub const BLOCK_HEADER_HASH_LEVEL_4: &str =
        "d8b52071506d67a7c79645f63d78f4ae83562ee57ff9bc8fcc383599bef03dfd";
    pub const BLOCK_HEADER_LEVEL_4: &str = "000000040143260c7a09634b82f3ba0f6d1f35b5fc3ea69d62aec9b11d4c07212f6c2da70f000000005fc4eba404cc4d723cbc771c5a76d3c53aad87c052287f9d6400435dc1bc5c1c64253c61c800000011000000010100000008000000000000000391668aee3882ec405849a45de206e4989b272d6b2d1cb06a68934eb541eb34900001c4907703ce66030000ed973ef6531c54dbe64852162df1afef1610157cf9ad0584b0bdb2677d857ebb33f9134720e984adf6b4c05721065378e07e199d2081a161648d8a22185af709";
    pub const BLOCK_HEADER_LEVEL_4_CONTEXT_HASH: &str =
        "CoVkLvDFs9tsuntBnDHSWMoWSGj33aK6e5kdAMrXDBjXkkFz67Ns";
    pub const BLOCK_HEADER_LEVEL_4_BLOCK_METADATA_HASH: &str =
        "bm4CzQ7hwDzX1QYd75xpDfHNgg3LFpasDHkMmjJWbNcZv8qLSdAh";
    pub const BLOCK_HEADER_LEVEL_4_OPERATION_METADATA_LIST_LIST_HASH: &str =
        "LLr2szUHVXLiEXTJPKajLN5aU5wwBpWSm9kMwnyeFGrREmMQP7Ens";

    pub fn block_header_level4_operations() -> Vec<Vec<String>> {
        vec![
            vec![
                "43260c7a09634b82f3ba0f6d1f35b5fc3ea69d62aec9b11d4c07212f6c2da70f0000000003f8d669480b0884c88bab52728dc785631f57f02cc09307ac54523c0700ab502b400106d278979248483167ee17575d44a6badc33375435913ced7bbf55d29b00".to_string(),
                "43260c7a09634b82f3ba0f6d1f35b5fc3ea69d62aec9b11d4c07212f6c2da70f00000000033624ce02c5feb3b9c00e8336ad6b9df6489f0c30f89df971e60e081bbb78c4e2417080801f2db1aee6c2db215e335685a6333d4f153f5feeaec1580794240b4d".to_string(),
                "43260c7a09634b82f3ba0f6d1f35b5fc3ea69d62aec9b11d4c07212f6c2da70f0000000003c71f8b007497736a394d7be642553bc7dd99e81ad04a4006c8fe81ee064433d391fd04447ed8d95593ce1a32f63dda844956fcdbb406884d4d5ef0451e6b190a".to_string(),
            ],
            vec![],
            vec![],
            vec![]
        ]
    }

    pub fn block_header_level4_operation_metadata_hashes() -> Option<Vec<Vec<OperationMetadataHash>>>
    {
        Some(vec![
            vec![
                "r3E9xb2QxUeG56eujC66B56CV8mpwjwfdVmEpYu3FRtuEx9tyfG"
                    .try_into()
                    .expect("Failed to decode hash"),
                "r3fqRzBrSWQ7U7kPXppiSrCUrFJss6J96XZddYjkCemT8hohQ7R"
                    .try_into()
                    .expect("Failed to decode hash"),
                "r49uWMVdKFmM3icKjtfVP9yywhLLAWANW1jMxZwN5MxRA7mC4tL"
                    .try_into()
                    .expect("Failed to decode hash"),
            ],
            vec![],
            vec![],
            vec![],
        ])
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
                    OperationsForBlock::new(
                        hex::decode(block_hash).unwrap().try_into().unwrap(),
                        4,
                    ),
                    Path::op(),
                    ops,
                )
            })
            .collect()
    }
}
