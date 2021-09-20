// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use crypto::hash::ProtocolHash;
use tezos_api::ffi::{
    ApplyBlockRequest, ApplyBlockRequestBuilder, ApplyBlockResponse, ForkingTestchainData,
    RustBytes, TezosRuntimeConfiguration,
};

use ocaml_interop::{OCamlRuntime, ToOCaml};
use tezos_interop::runtime;
use tezos_interop::{ffi, runtime::OCamlBlockPanic};
use tezos_messages::p2p::binary_message::BinaryRead;
use tezos_messages::p2p::encoding::prelude::*;

const CHAIN_ID: &str = "8eceda2f";
const HEADER: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";
const HEADER_HASH: &str = "61e687e852460b28f0f9540ccecf8f6cf87a5ad472c814612f0179caf4b9f673";
const OPERATION: &str = "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a";
const MAX_OPERATIONS_TTL: i32 = 5;

mod tezos_ffi {
    use ocaml_interop::ocaml;
    use tezos_api::ffi::{ApplyBlockRequest, ApplyBlockResponse};

    ocaml! {
        pub fn setup_benchmark_apply_block_response(response: ApplyBlockResponse);
        pub fn apply_block_request_decoded_roundtrip(
            request: ApplyBlockRequest,
        ) -> ApplyBlockResponse;
    }
}

fn init_bench_runtime() {
    // init runtime and turn on/off ocaml logging
    ffi::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        compute_context_action_tree_hashes: false,
        log_enabled: false,
    })
    .unwrap();
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
                OperationsForBlock::new(hex::decode(block_hash).unwrap().try_into().unwrap(), 4),
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

fn apply_block_request_decoded_roundtrip(
    request: ApplyBlockRequest,
) -> Result<(), OCamlBlockPanic> {
    runtime::execute(move |rt: &mut OCamlRuntime| {
        let request = request.to_boxroot(rt);
        let result = tezos_ffi::apply_block_request_decoded_roundtrip(rt, &request);
        let _response: ApplyBlockResponse = result.to_rust(rt);
    })
}

fn criterion_benchmark(c: &mut Criterion) {
    init_bench_runtime();

    let response_with_some_forking_data: ApplyBlockResponse = ApplyBlockResponse {
        validation_result_message: "validation_result_message".to_string(),
        context_hash: "CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE"
            .try_into()
            .expect("failed to convert"),
        block_header_proto_json: "block_header_proto_json".to_string(),
        protocol_hash: ProtocolHash::try_from(
            "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
        )
        .expect("failed to convert"),
        next_protocol_hash: ProtocolHash::try_from(
            "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
        )
        .expect("failed to convert"),
        block_header_proto_metadata_bytes: "block_header_proto_metadata_json".to_string().into(),
        operations_proto_metadata_bytes: vec![vec!["operations_proto_metadata_json"
            .to_string()
            .into()]],
        max_operations_ttl: 6,
        last_allowed_fork_level: 8,
        forking_testchain: true,
        forking_testchain_data: Some(ForkingTestchainData {
            test_chain_id: "NetXgtSLGNJvNye".try_into().unwrap(),
            forking_block_hash: "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET"
                .try_into()
                .unwrap(),
        }),
        block_metadata_hash: None,
        ops_metadata_hashes: None,
        ops_metadata_hash: None,
        cycle_rolls_owner_snapshots: vec![],
        new_protocol_constants_json: None,
        new_cycle_eras_json: None,
        commit_time: 1.0,
    };

    let _ignored = runtime::execute(move |rt: &mut OCamlRuntime| {
        let ocaml_response = response_with_some_forking_data.to_boxroot(rt);
        tezos_ffi::setup_benchmark_apply_block_response(rt, &ocaml_response);
    });

    c.bench_function("apply_block_request_decoded_roundtrip", |b| {
        let request: ApplyBlockRequest = ApplyBlockRequestBuilder::default()
            .chain_id(hex::decode(CHAIN_ID).unwrap().try_into().unwrap())
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

        b.iter(|| apply_block_request_decoded_roundtrip(black_box(request.clone())))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
