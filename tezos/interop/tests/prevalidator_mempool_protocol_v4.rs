// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serial_test::serial;

use crypto::hash::{ChainId, OperationHash};
use tezos_api::ffi::{
    ApplyBlockRequest, ApplyBlockResponse, BeginConstructionRequest, ClassifiedOperation,
    OperationClassification, ValidateOperationRequest, ValidateOperationResult,
};

use tezos_interop::apply_encoded_message;
use tezos_messages::p2p::binary_message::{BinaryRead, MessageHash};
use tezos_messages::p2p::encoding::prelude::*;
use tezos_protocol_ipc_messages::{NodeMessage, ProtocolMessage};

mod common;

// TODO: needs fixing, right now fails with
// FailedToApplyBlock { message: "[{\"kind\":\"permanent\",\"id\":\"ffi.inconsistent_operations_hash\",\"description\":\"expected: LLoavQa24zmPWVYx8TBbDiQQUrC9QqmczXJLUqyNdEDDormdFDkcR vs LLoa7bxRTKaQN2bLYoitYB6bU2DvLnBAqrVjZcvJ364cTcX2PZYKU\"}]" }
// probably because of a mismatch in the operations returned by `block_header_level2_operations`
#[ignore]
#[test]
#[serial]
fn test_begin_construction() -> Result<(), anyhow::Error> {
    common::init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) = common::init_test_protocol_context(
        "mempool_test_storage_01",
        test_data_protocol_v4::tezos_network(),
    );

    // apply block 1 and block 2
    let (last_block, _apply_block_result) = apply_blocks_1_2(&chain_id, genesis_block_header);
    let predecessor_hash = last_block
        .hash()
        .as_ref()
        .unwrap()
        .as_slice()
        .try_into()
        .unwrap();

    // let's initialize prevalidator for current head
    let prevalidator = apply_encoded_message(ProtocolMessage::BeginConstruction(
        BeginConstructionRequest {
            chain_id: chain_id.clone(),
            predecessor: last_block,
            predecessor_hash,
            protocol_data: None,
        },
    ))
    .unwrap();
    let prevalidator = expect_response!(BeginConstructionResult, prevalidator)?;
    assert_eq!(prevalidator.chain_id, chain_id);

    let operation =
        test_data_protocol_v4::operation_from_hex(test_data_protocol_v4::OPERATION_LEVEL_3);
    let operation_hash = operation.message_typed_hash::<OperationHash>().unwrap();

    let result = apply_encoded_message(ProtocolMessage::ValidateOperation(
        ValidateOperationRequest {
            prevalidator,
            operation_hash,
            operation,
        },
    ))
    .unwrap();
    let result = expect_response!(ValidateOperationResponse, result)?;
    assert_eq!(result.prevalidator.chain_id, chain_id);
    assert!(matches!(
        result.result,
        ValidateOperationResult::Classified(ClassifiedOperation {
            classification: OperationClassification::Applied,
            ..
        })
    ));

    Ok(())
}

fn apply_blocks_1_2(
    chain_id: &ChainId,
    genesis_block_header: BlockHeader,
) -> (BlockHeader, ApplyBlockResponse) {
    // apply first block - level 1
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v4::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            pred_header: genesis_block_header,
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v4::block_operations_from_hex(
                    test_data_protocol_v4::BLOCK_HEADER_HASH_LEVEL_1,
                    test_data_protocol_v4::block_header_level1_operations(),
                ),
            ),
            max_operations_ttl: 0,
            predecessor_block_metadata_hash: None,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();
    assert_eq!(
        test_data_protocol_v4::context_hash(
            test_data_protocol_v4::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH
        ),
        apply_block_result.context_hash
    );
    assert_eq!(1, apply_block_result.max_operations_ttl);

    // apply second block - level 2
    let apply_block_result =
        apply_encoded_message(ProtocolMessage::ApplyBlockCall(ApplyBlockRequest {
            chain_id: chain_id.clone(),
            block_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v4::BLOCK_HEADER_LEVEL_2).unwrap(),
            )
            .unwrap(),
            pred_header: BlockHeader::from_bytes(
                hex::decode(test_data_protocol_v4::BLOCK_HEADER_LEVEL_1).unwrap(),
            )
            .unwrap(),
            operations: ApplyBlockRequest::convert_operations(
                test_data_protocol_v4::block_operations_from_hex(
                    test_data_protocol_v4::BLOCK_HEADER_HASH_LEVEL_2,
                    test_data_protocol_v4::block_header_level2_operations(),
                ),
            ),
            max_operations_ttl: apply_block_result.max_operations_ttl,
            predecessor_block_metadata_hash: apply_block_result.block_metadata_hash,
            predecessor_ops_metadata_hash: None,
        }))
        .unwrap();
    let apply_block_result = expect_response!(ApplyBlockResult, apply_block_result).unwrap();
    assert_eq!(
        test_data_protocol_v4::context_hash(
            test_data_protocol_v4::BLOCK_HEADER_LEVEL_2_CONTEXT_HASH
        ),
        apply_block_result.context_hash
    );
    assert_eq!(
        "lvl 2, fit 1:1, prio 8, 0 ops",
        &apply_block_result.validation_result_message
    );
    assert_eq!(2, apply_block_result.max_operations_ttl);

    (
        BlockHeader::from_bytes(hex::decode(test_data_protocol_v4::BLOCK_HEADER_LEVEL_2).unwrap())
            .unwrap(),
        apply_block_result,
    )
}

/// Test data for protocol_v4 like 008 edo
mod test_data_protocol_v4 {
    use std::convert::TryFrom;

    use crypto::hash::{BlockHash, ContextHash};
    use tezos_api::environment::TezosEnvironmentConfiguration;
    use tezos_context_api::{GenesisChain, PatchContext, ProtocolOverrides};
    use tezos_messages::p2p::binary_message::BinaryRead;
    use tezos_messages::p2p::encoding::prelude::*;

    pub fn tezos_network() -> TezosEnvironmentConfiguration {
        TezosEnvironmentConfiguration {
            genesis: GenesisChain {
                time: "2022-01-25T15:00:00Z".to_string(),
                block: "BLockGenesisGenesisGenesisGenesisGenesis1db77eJNeJ9".to_string(),
                protocol: "Ps9mPmXaRzmzk35gbAYNCAw6UXdE2qoABTHbN2oEEc1qM7CwT9P".to_string(),
            },
            bootstrap_lookup_addresses: vec![
                "ithacanet.teztnets.xyz".to_string(),
                "ithacanet.smartpy.io".to_string(),
                "ithacanet.kaml.fr".to_string(),
                "ithacanet.boot.ecadinfra.com".to_string(),
            ],
            version: "TEZOS_ITHACANET_2022-01-25T15:00:00Z".to_string(),
            protocol_overrides: ProtocolOverrides {
                user_activated_upgrades: vec![
                    (
                        8191_i32,
                        "Psithaca2MLRFYargivpo7YvUr7wUDqyxrdhC5CQq78mRvimz6A".to_string(),
                    ),
                ],
                user_activated_protocol_overrides: vec![],
            },
            enable_testchain: true,
            patch_context_genesis_parameters: Some(PatchContext {
                key: "sandbox_parameter".to_string(),
                json: r#"{ "genesis_pubkey": "edpkuYLienS3Xdt5c1vfRX1ibMxQuvfM67ByhJ9nmRYYKGAAoTq1UC" }"#.to_string(),
            }),
        }
    }

    pub fn context_hash(hash: &str) -> ContextHash {
        ContextHash::from_base58_check(hash).unwrap()
    }

    // Raw headers are obtained with:
    // curl -s https://testnet-tezos.giganode.io/chains/main/blocks/2/header/raw

    // Encoded block header hashes with:
    // ./tezos-codec encode alpha.operation.raw from '{ "branch": "BLAuTiHQdSCXXhfDvv417JKsM8bYT5tNsrpmWyJiMrMdYJNuoLP", "data": "" }'

    // BLAuTiHQdSCXXhfDvv417JKsM8bYT5tNsrpmWyJiMrMdYJNuoLP
    pub const BLOCK_HEADER_HASH_LEVEL_1: &str =
        "3c73510fa502e628b413bb49458aec38a53409e11f8d9ec72212deb813c63ff9";
    pub const BLOCK_HEADER_LEVEL_1: &str =
        include_str!("resources/ithaca_block_header_level1.bytes");
    pub const BLOCK_HEADER_LEVEL_1_CONTEXT_HASH: &str =
        "CoV12qJeHtk6V7XvjxSFczupR4hnC9KFob1HV3Wj1nMHJ3ErkNK4";

    pub fn block_header_level1_operations() -> Vec<Vec<String>> {
        vec![]
    }

    // BMQHBqRo1ZykVpEfKi7ruJa1iJKLV1sLAvwBR3zKWAxMXmVNjTR
    pub const BLOCK_HEADER_HASH_LEVEL_2: &str =
        "de83e3d7960559f4724302b1981370cf8e8b4db1256639bbb57e901c39662fc8";
    pub const BLOCK_HEADER_LEVEL_2: &str = "00000002013c73510fa502e628b413bb49458aec38a53409e11f8d9ec72212deb813c63ff90000000061f012d304d27c29612d539fdfcdbbbcc8e75270542902e9a889c08d8d6d5db0b9174bc98000000011000000010100000008000000000000000192122778fe230fa9c415ebfc0f4d15d34e9d204c8df20be1ba930473851a3f9100037985fafe3403090000005f118cf48a158565ed4c54a284dd223958eb335c957624c307e26122bea7952d13dfcc5bc8446cf5e396d61fda3c1c8a0b36165d887fd03a2ec671c8d4c61805";
    pub const BLOCK_HEADER_LEVEL_2_CONTEXT_HASH: &str =
        "CoVke3NJm3mGT86tCp6Aa1V9iryBCkD2JpZUvaqybaiqTih7D5ad";

    pub fn block_header_level2_operations() -> Vec<Vec<String>> {
        vec![vec![], vec![], vec![], vec![]]
    }

    // Extracted from https://testnet-tezos.giganode.io/chains/main/blocks/3/operations/0/0
    // and encoded with `tezos-codec encode 011-PtHangz2.operation from ./endorsement.json`.
    // JSON:
    // {
    //   "branch": "BMQHBqRo1ZykVpEfKi7ruJa1iJKLV1sLAvwBR3zKWAxMXmVNjTR",
    //   "contents": [{
    //     "kind": "endorsement",
    //     "level": 2
    //   }],
    //   "signature": "sigZZe9DthzsaovGYtXWaK6jG99KptAZWKnRJzZR7rWLJNn1MvNaV53hY1qZzSt1UtBFANbqkJgBmFiTGqa8fFQsn9X3GDQH"
    // }
    pub const OPERATION_LEVEL_3: &str = "de83e3d7960559f4724302b1981370cf8e8b4db1256639bbb57e901c39662fc8000000000258796116508e85f35ab9e8a1d1e24478e4bb13072f1a3a5f3f501f7083afa6f0d6d13f287b48a955ad6884eaa06576353c305287f76adc23305ae7ac5c05eb02";

    pub fn operation_from_hex(bytes: &str) -> Operation {
        Operation::from_bytes(hex::decode(bytes).unwrap()).unwrap()
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
                        BlockHash::try_from(hex::decode(block_hash).unwrap()).unwrap(),
                        4,
                    ),
                    Path::op(),
                    ops,
                )
            })
            .collect()
    }
}
