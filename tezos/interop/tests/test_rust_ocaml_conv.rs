// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};

use crypto::hash::{BlockHash, ChainId, ContextHash, HashType, OperationHash, ProtocolHash};
use ocaml_interop::{OCaml, OCamlRuntime, ToOCaml};
use serial_test::serial;

use tezos_api::ffi::{
    ApplyBlockRequest, ApplyBlockRequestBuilder, ApplyBlockResponse, BeginConstructionRequest,
    CycleRollsOwnerSnapshot, ForkingTestchainData, PrevalidatorWrapper, ProtocolRpcRequest,
    RpcMethod, RpcRequest, RustBytes, ValidateOperationRequest,
};
use tezos_conv::*;
use tezos_interop::runtime;
use tezos_messages::p2p::{
    binary_message::BinaryRead, encoding::block_header::BlockHeader,
    encoding::operation::Operation, encoding::operations_for_blocks::OperationsForBlock,
    encoding::operations_for_blocks::OperationsForBlocksMessage,
    encoding::operations_for_blocks::Path,
};

const CHAIN_ID: &str = "8eceda2f";
const HEADER: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";
const HEADER_HASH: &str = "61e687e852460b28f0f9540ccecf8f6cf87a5ad472c814612f0179caf4b9f673";
const OPERATION_HASH: &str = "7e73e3da041ea251037af062b7bc04b37a5ee38bc7e229e7e20737071ed73af4";
const OPERATION: &str = "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a";
const MAX_OPERATIONS_TTL: i32 = 5;

#[allow(clippy::too_many_arguments)]
mod tezos_ffi {
    use ocaml_interop::{
        ocaml, OCamlBytes, OCamlFloat, OCamlInt, OCamlInt32, OCamlInt64, OCamlList,
    };

    use tezos_conv::*;

    ocaml! {
        pub fn construct_and_compare_hash(operation_hash: OCamlOperationHash, hash_bytes: OCamlBytes) -> bool;
        pub fn construct_and_compare_block_header(
            block_header: OCamlBlockHeader,
            level: OCamlInt32,
            proto_level: OCamlInt,
            validation_passes: OCamlInt,
            timestamp: OCamlInt64,
            predecessor:  OCamlBlockHash,
            operations_hash: OCamlOperationListListHash,
            fitness: OCamlList<OCamlBytes>,
            context: OCamlContextHash,
            protocol_data: OCamlBytes,
        ) -> bool;
        pub fn construct_and_compare_apply_block_request(
            apply_block_request: OCamlApplyBlockRequest,
            chain_id: OCamlChainId,
            block_header: OCamlBlockHeader,
            pred_header: OCamlBlockHeader,
            max_operations_ttl: OCamlInt,
            operations: OCamlList<OCamlList<OCamlOperation>>,
        ) -> bool;
        pub fn construct_and_compare_cycle_rolls_owner_snapshot(
            cycle_rolls_owner_snapshot: OCamlCycleRollsOwnerSnapshot,
            cycle: OCamlInt,
            seed_bytes: OCamlBytes,
            rolls_data: OCamlList<(OCamlBytes, OCamlList<OCamlInt>)>,
            last_roll: OCamlInt32,
        ) -> bool;
        pub fn construct_and_compare_apply_block_response(
            apply_block_response: OCamlApplyBlockResponse,
            validation_result_message: OCamlBytes,
            context_hash: OCamlContextHash,
            protocol_hash: OCamlProtocolHash,
            next_protocol_hash: OCamlProtocolHash,
            block_header_proto_json: OCamlBytes,
            block_header_proto_metadata_bytes: OCamlBytes,
            operations_proto_metadata_bytes: OCamlList<OCamlList<OCamlBytes>>,
            max_operations_ttl: OCamlInt,
            last_allowed_fork_level: OCamlInt32,
            forking_testchain: bool,
            forking_testchain_data: Option<OCamlForkingTestchainData>,
            block_metadata_hash: Option<OCamlBlockMetadataHash>,
            ops_metadata_hashes: Option<OCamlList<OCamlList<OCamlOperationMetadataHash>>>,
            ops_metadata_hash: Option<OCamlOperationMetadataListListHash>,
            cycle_rolls_owner_snapshots: OCamlList<OCamlCycleRollsOwnerSnapshot>,
            new_protocol_constants_json: Option<String>,
            new_cycle_eras_json: Option<String>,
            commit_time: OCamlFloat,
            execution_timestamps: OCamlApplyBlockExecutionTimestamps,
        ) -> bool;
        pub fn construct_and_compare_begin_construction_request(
            begin_construction_request: OCamlBeginConstructionRequest,
            chain_id: OCamlChainId,
            predecessor: OCamlBlockHeader,
            predecessor_hash: OCamlBlockHash,
            protocol_data: Option<OCamlBytes>,
        ) -> bool;
        pub fn construct_and_compare_validate_operation_request(
            validate_operation_request: OCamlValidateOperationRequest,
            prevalidator: OCamlPrevalidatorWrapper,
            operation: OCamlOperation,
        ) -> bool;
        pub fn construct_and_compare_rpc_request(
            rpc_request: OCamlRpcRequest,
            body: OCamlBytes,
            context_path: OCamlBytes,
            meth: OCamlRpcMethod,
            content_type: Option<OCamlBytes>,
            accept: Option<OCamlBytes>,
        ) -> bool;
        pub fn construct_and_compare_protocol_rpc_request(
            protocol_rpc_request: OCamlProtocolRpcRequest,
            block_header: OCamlBlockHeader,
            chain_id: OCamlChainId,
            chain_arg: OCamlBytes,
            request: OCamlRpcRequest,
        ) -> bool;
        pub fn construct_and_compare_operation(
            operation: OCamlOperation,
            branch: OCamlBlockHash,
            proto: OCamlBytes,
        ) -> bool;
        pub fn construct_and_compare_prevalidator_wrapper(
            prevalidator_wrapper: OCamlPrevalidatorWrapper,
            chain_id: OCamlChainId,
            protocol: OCamlProtocolHash,
            context_fitness: Option<OCamlList<OCamlBytes>>,
            predecessor: OCamlBlockHash,
        ) -> bool;
    }
}

fn block_operations_from_hex(
    block_hash: &str,
    hex_operations: Vec<Vec<RustBytes>>,
) -> Vec<OperationsForBlocksMessage> {
    hex_operations
        .into_iter()
        .map(|bo| {
            let ops = bo
                .into_iter()
                .map(|op| Operation::from_bytes(op).unwrap())
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

fn sample_operations_for_request_decoded() -> Vec<Vec<RustBytes>> {
    vec![
        vec![hex::decode(OPERATION).unwrap()],
        vec![],
        vec![],
        vec![hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08").unwrap()],
        vec![]
    ]
}

#[test]
#[serial]
fn test_hash_conv() {
    let operation_hash = OperationHash::try_from(hex::decode(OPERATION_HASH).unwrap()).unwrap();

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let hash_bytes = operation_hash.as_ref().to_boxroot(rt);
        let operation_hash = operation_hash.to_boxroot(rt);
        tezos_ffi::construct_and_compare_hash(rt, &operation_hash, &hash_bytes).to_rust(rt)
    })
    .unwrap();

    assert!(result, "OperationHash conversion failed")
}

#[test]
#[serial]
fn test_block_header_conv() {
    let block_header = BlockHeader::from_bytes(hex::decode(HEADER).unwrap()).unwrap();

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let level = block_header.level().to_boxroot(rt);
        let proto_level = OCaml::of_i32(block_header.proto() as i32);
        let validation_passes = OCaml::of_i32(block_header.validation_pass() as i32);
        let timestamp = block_header.timestamp().i64().to_boxroot(rt);
        let predecessor = block_header.predecessor().to_boxroot(rt);
        let operations_hash = block_header.operations_hash().to_boxroot(rt);
        let fitness = block_header.fitness().as_ref().to_boxroot(rt);
        let context = block_header.context().to_boxroot(rt);
        let protocol_data = AsRef::<Vec<u8>>::as_ref(&block_header.protocol_data()).to_boxroot(rt);
        let block_header = FfiBlockHeader::from(&block_header).to_boxroot(rt);

        tezos_ffi::construct_and_compare_block_header(
            rt,
            &block_header,
            &level,
            &proto_level,
            &validation_passes,
            &timestamp,
            &predecessor,
            &operations_hash,
            &fitness,
            &context,
            &protocol_data,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "BlockHeader conversion failed")
}

#[test]
#[serial]
fn test_apply_block_request_conv() {
    let request: ApplyBlockRequest = ApplyBlockRequestBuilder::default()
        .chain_id(ChainId::try_from(hex::decode(CHAIN_ID).unwrap()).unwrap())
        .block_header(BlockHeader::from_bytes(hex::decode(HEADER).unwrap()).unwrap())
        .pred_header(BlockHeader::from_bytes(hex::decode(HEADER).unwrap()).unwrap())
        .max_operations_ttl(MAX_OPERATIONS_TTL)
        .operations(ApplyBlockRequest::convert_operations(
            block_operations_from_hex(HEADER_HASH, sample_operations_for_request_decoded()),
        ))
        .predecessor_block_metadata_hash(None)
        .predecessor_ops_metadata_hash(None)
        .build()
        .unwrap();

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let ffi_operations: Vec<Vec<FfiOperation>> = request
            .operations
            .iter()
            .map(|ops| ops.iter().map(FfiOperation::from).collect())
            .collect();

        let apply_block_request = request.to_boxroot(rt);
        let chain_id = request.chain_id.to_boxroot(rt);
        let block_header = FfiBlockHeader::from(&request.block_header).to_boxroot(rt);
        let pred_header = FfiBlockHeader::from(&request.pred_header).to_boxroot(rt);
        let max_operations_ttl = OCaml::of_i32(request.max_operations_ttl);
        let operations = ffi_operations.to_boxroot(rt);

        tezos_ffi::construct_and_compare_apply_block_request(
            rt,
            &apply_block_request,
            &chain_id,
            &block_header,
            &pred_header,
            &max_operations_ttl,
            &operations,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "ApplyBlockRequest conversion failed")
}

#[test]
#[serial]
fn test_cycle_rolls_owner_snapshot_conv() {
    let data = CycleRollsOwnerSnapshot {
        cycle: 1,
        seed_bytes: vec![1, 2, 3, 4],
        rolls_data: vec![],
        last_roll: 200,
    };

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let cycle_rolls_owner_snapshot = data.to_boxroot(rt);
        let cycle = data.cycle.to_boxroot(rt);
        let seed_bytes = data.seed_bytes.to_boxroot(rt);
        let rolls_data = data.rolls_data.to_boxroot(rt);
        let last_roll = data.last_roll.to_boxroot(rt);

        tezos_ffi::construct_and_compare_cycle_rolls_owner_snapshot(
            rt,
            &cycle_rolls_owner_snapshot,
            &cycle,
            &seed_bytes,
            &rolls_data,
            &last_roll,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "ApplyBlockRequest conversion failed")
}

#[test]
#[serial]
fn test_apply_block_response_conv() {
    let response = ApplyBlockResponse {
        validation_result_message: "validation_result_message".to_string(),
        context_hash: ContextHash::try_from("CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE")
            .expect("failed to convert"),
        protocol_hash: ProtocolHash::try_from(
            "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
        )
        .expect("failed to convert"),
        next_protocol_hash: ProtocolHash::try_from(
            "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
        )
        .expect("failed to convert"),
        block_header_proto_json: "block_header_proto_json".to_string(),
        block_header_proto_metadata_bytes: "block_header_proto_metadata_json".to_string().into(),
        operations_proto_metadata_bytes: vec![vec!["operations_proto_metadata_json"
            .to_string()
            .into()]],
        max_operations_ttl: 6,
        last_allowed_fork_level: 8,
        forking_testchain: true,
        forking_testchain_data: Some(ForkingTestchainData {
            test_chain_id: ChainId::try_from("NetXgtSLGNJvNye").unwrap(),
            forking_block_hash: BlockHash::try_from(
                "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET",
            )
            .unwrap(),
        }),
        block_metadata_hash: None,
        ops_metadata_hashes: None,
        ops_metadata_hash: None,
        cycle_rolls_owner_snapshots: vec![],
        new_protocol_constants_json: None,
        new_cycle_eras_json: None,
        commit_time: 1.0,
        execution_timestamps: Default::default(),
    };

    let (into, from): (bool, bool) = runtime::execute(move |rt: &mut OCamlRuntime| {
        let apply_block_response = response.to_boxroot(rt);
        let validation_result_message = response.validation_result_message.to_boxroot(rt);
        let context_hash = response.context_hash.to_boxroot(rt);
        let protocol_hash = response.protocol_hash.to_boxroot(rt);
        let next_protocol_hash = response.next_protocol_hash.to_boxroot(rt);
        let block_header_proto_json = response.block_header_proto_json.to_boxroot(rt);
        let block_header_proto_metadata_bytes =
            response.block_header_proto_metadata_bytes.to_boxroot(rt);
        let operations_proto_metadata_bytes =
            response.operations_proto_metadata_bytes.to_boxroot(rt);
        let max_operations_ttl = OCaml::of_i32(response.max_operations_ttl);
        let last_allowed_fork_level = response.last_allowed_fork_level.to_boxroot(rt);
        let forking_testchain = response.forking_testchain.to_boxroot(rt);
        let forking_testchain_data = response.forking_testchain_data.to_boxroot(rt);
        let block_metadata_hash = response.block_metadata_hash.to_boxroot(rt);
        let ops_metadata_hashes = response.ops_metadata_hashes.to_boxroot(rt);
        let ops_metadata_hash = response.ops_metadata_hash.to_boxroot(rt);
        let cycle_rolls_owner_snapshots = response.cycle_rolls_owner_snapshots.to_boxroot(rt);
        let new_protocol_constants_json = response.new_protocol_constants_json.to_boxroot(rt);
        let new_cycle_eras_json = response.new_cycle_eras_json.to_boxroot(rt);
        let commit_time = response.commit_time.to_boxroot(rt);
        let execution_timestamps = response.execution_timestamps.to_boxroot(rt);

        let into_result: bool = tezos_ffi::construct_and_compare_apply_block_response(
            rt,
            &apply_block_response,
            &validation_result_message,
            &context_hash,
            &protocol_hash,
            &next_protocol_hash,
            &block_header_proto_json,
            &block_header_proto_metadata_bytes,
            &operations_proto_metadata_bytes,
            &max_operations_ttl,
            &last_allowed_fork_level,
            &forking_testchain,
            &forking_testchain_data,
            &block_metadata_hash,
            &ops_metadata_hashes,
            &ops_metadata_hash,
            &cycle_rolls_owner_snapshots,
            &new_protocol_constants_json,
            &new_cycle_eras_json,
            &commit_time,
            &execution_timestamps,
        )
        .to_rust(rt);

        let rust_apply_block_response: ApplyBlockResponse = apply_block_response.to_rust(rt);
        let from_result = rust_apply_block_response == response;

        (into_result, from_result)
    })
    .unwrap();

    assert!(into, "ApplyBlockResponse conversion into OCaml failed");
    assert!(from, "ApplyBlockResponse conversion from OCaml failed");
}

#[test]
#[serial]
fn test_begin_construction_request_conv() {
    let predecessor = BlockHeader::from_bytes(hex::decode(HEADER).unwrap()).unwrap();
    let predecessor_hash = predecessor
        .hash()
        .as_ref()
        .unwrap()
        .as_slice()
        .try_into()
        .unwrap();
    let begin_construction_request = BeginConstructionRequest {
        chain_id: ChainId::try_from(hex::decode(CHAIN_ID).unwrap()).unwrap(),
        predecessor,
        predecessor_hash,
        protocol_data: Some(vec![1, 2, 3, 4, 5, 6, 7, 8]),
        predecessor_block_metadata_hash: None,
        predecessor_ops_metadata_hash: None,
    };

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let chain_id = begin_construction_request.chain_id.to_boxroot(rt);
        let predecessor =
            FfiBlockHeader::from(&begin_construction_request.predecessor).to_boxroot(rt);
        let predecessor_hash = begin_construction_request.predecessor_hash.to_boxroot(rt);
        let protocol_data = begin_construction_request.protocol_data.to_boxroot(rt);
        let begin_construction_request = begin_construction_request.to_boxroot(rt);

        tezos_ffi::construct_and_compare_begin_construction_request(
            rt,
            &begin_construction_request,
            &chain_id,
            &predecessor,
            &predecessor_hash,
            &protocol_data,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "BeginConstructionRequest conversion failed")
}

fn get_protocol_hash(prefix: &[u8]) -> ProtocolHash {
    let mut vec = prefix.to_vec();
    vec.extend(std::iter::repeat(0).take(HashType::ProtocolHash.size() - prefix.len()));
    vec.try_into().unwrap()
}

#[test]
#[serial]
fn test_validate_operation_request_conv() {
    let prevalidator = PrevalidatorWrapper {
        chain_id: ChainId::try_from(hex::decode(CHAIN_ID).unwrap()).unwrap(),
        protocol: get_protocol_hash(&[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        context_fitness: Some(vec![vec![0, 1], vec![0, 0, 1, 2, 3, 4, 5]]),
        predecessor: BlockHash::try_from("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")
            .unwrap(),
    };
    let operations = ApplyBlockRequest::convert_operations(block_operations_from_hex(
        HEADER_HASH,
        sample_operations_for_request_decoded(),
    ));
    let operation = operations[0][0].clone();
    let validate_operation_request = ValidateOperationRequest {
        prevalidator,
        operation,
    };

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let prevalidator = validate_operation_request.prevalidator.to_boxroot(rt);
        let operation = FfiOperation::from(&validate_operation_request.operation).to_boxroot(rt);
        let validate_operation_request = validate_operation_request.to_boxroot(rt);

        tezos_ffi::construct_and_compare_validate_operation_request(
            rt,
            &validate_operation_request,
            &prevalidator,
            &operation,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "ValidateOperationRequest conversion failed")
}

#[test]
#[serial]
fn test_validate_rpc_request_conv() {
    let rpc_request = RpcRequest {
        body: "body of request".to_owned(),
        context_path: "/context/path/string".to_owned(),
        meth: RpcMethod::GET,
        content_type: None,
        accept: None,
    };
    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let body = rpc_request.body.to_boxroot(rt);
        let context_path = rpc_request.context_path.to_boxroot(rt);
        let meth = rpc_request.meth.to_boxroot(rt);
        let content_type = rpc_request.content_type.to_boxroot(rt);
        let accept = rpc_request.accept.to_boxroot(rt);
        let rpc_request = rpc_request.to_boxroot(rt);
        tezos_ffi::construct_and_compare_rpc_request(
            rt,
            &rpc_request,
            &body,
            &context_path,
            &meth,
            &content_type,
            &accept,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "RpcRequest conversion failed")
}

#[test]
#[serial]
fn test_validate_protocol_rpc_request_conv() {
    let rpc_request = RpcRequest {
        body: "body of request".to_owned(),
        context_path: "/context/path/string".to_owned(),
        meth: RpcMethod::GET,
        content_type: None,
        accept: None,
    };
    let protocol_rpc_request = ProtocolRpcRequest {
        block_header: BlockHeader::from_bytes(hex::decode(HEADER).unwrap()).unwrap(),
        chain_arg: "some chain arg".to_owned(),
        chain_id: ChainId::try_from(hex::decode(CHAIN_ID).unwrap()).unwrap(),
        request: rpc_request,
    };
    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let block_header = FfiBlockHeader::from(&protocol_rpc_request.block_header).to_boxroot(rt);
        let chain_arg = protocol_rpc_request.chain_arg.to_boxroot(rt);
        let chain_id = protocol_rpc_request.chain_id.to_boxroot(rt);
        let request = protocol_rpc_request.request.to_boxroot(rt);
        let protocol_rpc_request = protocol_rpc_request.to_boxroot(rt);

        tezos_ffi::construct_and_compare_protocol_rpc_request(
            rt,
            &protocol_rpc_request,
            &block_header,
            &chain_id,
            &chain_arg,
            &request,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "ProtocolRpcRequest conversion failed")
}

#[test]
#[serial]
fn test_validate_operation_conv() {
    let operations = ApplyBlockRequest::convert_operations(block_operations_from_hex(
        HEADER_HASH,
        sample_operations_for_request_decoded(),
    ));
    let operation = operations[0][0].clone();

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let branch = operation.branch().to_boxroot(rt);
        let proto = AsRef::<Vec<u8>>::as_ref(operation.data()).to_boxroot(rt);
        let operation = FfiOperation::from(&operation).to_boxroot(rt);
        tezos_ffi::construct_and_compare_operation(rt, &operation, &branch, &proto).to_rust(rt)
    })
    .unwrap();

    assert!(result, "Operation conversion failed")
}

#[test]
#[serial]
fn test_validate_prevalidator_wrapper_conv() {
    let prevalidator_wrapper = PrevalidatorWrapper {
        chain_id: ChainId::try_from(hex::decode(CHAIN_ID).unwrap()).unwrap(),
        protocol: get_protocol_hash(&[1, 2, 3, 4, 5, 6, 7, 8, 9]),
        context_fitness: Some(vec![vec![0, 0], vec![0, 0, 1, 2, 3, 4, 5]]),
        predecessor: BlockHash::try_from("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")
            .unwrap(),
    };

    let result: bool = runtime::execute(move |rt: &mut OCamlRuntime| {
        let chain_id = prevalidator_wrapper.chain_id.to_boxroot(rt);
        let protocol = prevalidator_wrapper.protocol.to_boxroot(rt);
        let context_fitness = prevalidator_wrapper.context_fitness.to_boxroot(rt);
        let predecessor = prevalidator_wrapper.predecessor.to_boxroot(rt);
        let prevalidator_wrapper = prevalidator_wrapper.to_boxroot(rt);
        tezos_ffi::construct_and_compare_prevalidator_wrapper(
            rt,
            &prevalidator_wrapper,
            &chain_id,
            &protocol,
            &context_fitness,
            &predecessor,
        )
        .to_rust(rt)
    })
    .unwrap();

    assert!(result, "PrevalidatorWrapper conversion failed")
}
