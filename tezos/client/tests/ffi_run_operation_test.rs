// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serial_test::serial;

use crypto::hash::{ChainId, ProtocolHash};
use tezos_api::environment::{OPERATION_LIST_LIST_HASH_EMPTY, TEZOS_ENV, TezosEnvironmentConfiguration};
use tezos_api::ffi::{InitProtocolContextResult, TezosRuntimeConfiguration};
use tezos_client::client;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

mod common;

fn init_test_runtime() {
    // init runtime and turn on/off ocaml logging
    client::change_runtime_configuration(
        TezosRuntimeConfiguration {
            log_enabled: common::is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
            debug_mode: false,
        }
    ).unwrap();
}

fn init_test_protocol_context(dir_name: &str) -> (ChainId, BlockHeader, ProtocolHash, InitProtocolContextResult) {
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV.get(&test_data::TEZOS_NETWORK).expect("no tezos environment configured");

    let result = client::init_protocol_context(
        common::prepare_empty_dir(dir_name),
        tezos_env.genesis.clone(),
        tezos_env.protocol_overrides.clone(),
        true,
        false,
        false,
        Some(test_data::get_patch_context()),
    ).unwrap();

    let genesis_commit_hash = match result.clone().genesis_commit_hash {
        None => panic!("we needed commit_genesis and here should be result of it"),
        Some(cr) => cr
    };

    (
        tezos_env.main_chain_id().expect("invalid chain id"),
        tezos_env.genesis_header(
            genesis_commit_hash,
            OPERATION_LIST_LIST_HASH_EMPTY.clone(),
        ).expect("genesis header error"),
        tezos_env.genesis_protocol().expect("protocol_hash error"),
        result
    )
}

#[test]
#[serial]
fn test_run_operations() {
    init_test_runtime();

    // init empty context for test
    let (chain_id, genesis_block_header, ..) = init_test_protocol_context("run_operation_test_storage_01");

    // apply block 1
    let last_block = apply_blocks_1(&chain_id, &genesis_block_header);
    assert!(true);

    // prepare encoded operation to send
    let operation = test_data::operation_from_hex(test_data::OPERATION_LEVEL_2);

    // TODO: FFI call for run_operation goes here
    // 

    // mock data for now
    // replace with actual json returned from ffi
    let result = r#"{ "contents":
    [ { "kind": "transaction",
        "source": "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx", "fee": "0",
        "counter": "1", "gas_limit": "1040000", "storage_limit": "60000",
        "amount": "1000000",
        "destination": "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
        "metadata":
          { "balance_updates": [],
            "operation_result":
              { "status": "applied",
                "balance_updates":
                  [ { "kind": "contract",
                      "contract": "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
                      "change": "-1000000" },
                    { "kind": "contract",
                      "contract": "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
                      "change": "1000000" } ], "consumed_gas": "10207" } } } ] }"#;

    assert_eq!(result, test_data::RUN_OPERTION_RESULT);

}

fn apply_blocks_1(chain_id: &ChainId, genesis_block_header: &BlockHeader) -> BlockHeader {
    // apply first block - level 1
    let apply_block_result = client::apply_block(
        &chain_id,
        &BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap()).unwrap(),
        &genesis_block_header,
        &test_data::block_operations_from_hex(
            test_data::BLOCK_HEADER_HASH_LEVEL_1,
            test_data::block_header_level1_operations(),
        ),
        0,
    ).unwrap();
    assert_eq!(test_data::context_hash(test_data::BLOCK_HEADER_LEVEL_1_CONTEXT_HASH), apply_block_result.context_hash);
    assert_eq!(1, apply_block_result.max_operations_ttl);


    BlockHeader::from_bytes(hex::decode(test_data::BLOCK_HEADER_LEVEL_1).unwrap()).unwrap()
}

mod test_data {
    use crypto::hash::{ContextHash, HashType};
    use tezos_api::environment::TezosEnvironment;
    use tezos_api::ffi::PatchContext;
    use tezos_messages::p2p::binary_message::BinaryMessage;
    use tezos_messages::p2p::encoding::prelude::*;

    pub const TEZOS_NETWORK: TezosEnvironment = TezosEnvironment::Sandbox;

    pub fn context_hash(hash: &str) -> ContextHash {
        HashType::ContextHash
            .string_to_bytes(hash)
            .unwrap()
    }

    // BMPtRJqFGQJRTfn8bXQR2grLE1M97XnUmG5vgjHMW7St1Wub7Cd
    pub const BLOCK_HEADER_HASH_LEVEL_1: &str = "cf764612821498403dd6bd71e2fcbde9ad2c50670113dadc15ec52d167c66b31";
    pub const BLOCK_HEADER_LEVEL_1: &str = include_str!("resources/sandbox_block_header_level1.bytes");
    pub const BLOCK_HEADER_LEVEL_1_CONTEXT_HASH: &str = "CoUozrLGjbckFtx3PNhCz1KjacPL8CDEfzZ1WQAruSZLpUgwN378";

    pub const PATCH_CONTEXT: &str = include_str!("../../../light_node/etc/tezedge_sandbox/sandbox-patch-context.json");

    pub const RUN_OPERTION_RESULT: &str = r#"{ "contents":
    [ { "kind": "transaction",
        "source": "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx", "fee": "0",
        "counter": "1", "gas_limit": "1040000", "storage_limit": "60000",
        "amount": "1000000",
        "destination": "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
        "metadata":
          { "balance_updates": [],
            "operation_result":
              { "status": "applied",
                "balance_updates":
                  [ { "kind": "contract",
                      "contract": "tz1KqTpEZ7Yob7QbPE4Hy4Wo8fHG8LhKxZSx",
                      "change": "-1000000" },
                    { "kind": "contract",
                      "contract": "tz1gjaF81ZRRvdzjobyfVNsAeSC6PScjfQwN",
                      "change": "1000000" } ], "consumed_gas": "10207" } } } ] }"#;

    pub fn block_header_level1_operations() -> Vec<Vec<String>> {
        vec![]
    }

    // this is the injected one (with fee set)
    // pub const OPERATION_LEVEL_2: &str = "cf764612821498403dd6bd71e2fcbde9ad2c50670113dadc15ec52d167c66b316c0002298c03ed7d454a101eb7022bc95f7e5f41ac78810a01c35000c0843d0000e7670f32038107a59a2b9cfefae36ea21f5aa63c001034e0ad237c810ea3df13e09516e48b21bcd384e53cc5c85cadabfd7d98822a5645e4e624955c41789b1649cd5167205a3941f9f579f575fadcb0a16ae7140e";

    // this should be the operation hash with a zero signature and zero fee (this should be accepted by the run_operation ffi)
    pub const OPERATION_LEVEL_2: &str = "cf764612821498403dd6bd71e2fcbde9ad2c50670113dadc15ec52d167c66b316c0002298c03ed7d454a101eb7022bc95f7e5f41ac78000180bd3fe0d403c0843d0000e7670f32038107a59a2b9cfefae36ea21f5aa63c0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000";
    pub fn operation_from_hex(bytes: &str) -> Operation {
        Operation::from_bytes(hex::decode(bytes).unwrap()).unwrap()
    }


    pub fn block_operations_from_hex(block_hash: &str, hex_operations: Vec<Vec<String>>) -> Vec<Option<OperationsForBlocksMessage>> {
        hex_operations
            .into_iter()
            .map(|bo| {
                let ops = bo
                    .into_iter()
                    .map(|op| Operation::from_bytes(hex::decode(op).unwrap()).unwrap())
                    .collect();
                Some(OperationsForBlocksMessage::new(OperationsForBlock::new(hex::decode(block_hash).unwrap(), 4), Path::Op, ops))
            })
            .collect()
    }

    pub fn get_patch_context() -> PatchContext {
        PatchContext {
            key: "sandbox_parameter".to_string(),
            json: PATCH_CONTEXT.to_string(),
        }
    }
}