// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use tezos_api::{
    ffi::{RustBytes, TezosRuntimeConfiguration},
    ocaml_conv::to_ocaml::{FfiBlockHeader, FfiOperation},
};

use tezos_interop::ffi;
use tezos_interop::runtime;
use tezos_interop::runtime::OcamlError;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;
use znfe::{ocaml_call, ocaml_frame, to_ocaml, IntoRust, OCaml, OCamlBytes, OCamlList, ToOCaml};

const CHAIN_ID: &str = "8eceda2f";
const HEADER: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";
const OPERATION: &str = "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a";

mod tezos_ffi {
    use tezos_api::ffi::{ApplyBlockRequest, ApplyBlockResponse};
    use tezos_messages::p2p::encoding::{block_header::BlockHeader, operation::Operation};
    use znfe::{ocaml, OCamlBytes, OCamlList};

    ocaml! {
        pub fn apply_block_params_roundtrip(chain_id: OCamlBytes, block_header: OCamlBytes, operations: OCamlList<OCamlList<OCamlBytes>>) -> (OCamlBytes, OCamlBytes, OCamlList<OCamlList<OCamlBytes>>);
        pub fn apply_block_params_decoded_roundtrip(chain_id: OCamlBytes, block_header: BlockHeader, operations: OCamlList<OCamlList<Operation>>) -> (OCamlBytes, BlockHeader, OCamlList<OCamlList<Operation>>);
        pub fn apply_block_request_roundtrip(data: OCamlBytes) -> OCamlBytes;
        pub fn apply_block_request_decoded_roundtrip(request: ApplyBlockRequest) -> ApplyBlockRequest;
        pub fn apply_block_response_roundtrip(data: OCamlBytes) -> OCamlBytes;
        pub fn apply_block_response_decoded_roundtrip(response: ApplyBlockResponse) -> ApplyBlockResponse;
    }
}

fn init_bench_runtime() {
    // init runtime and turn on/off ocaml logging
    ffi::change_runtime_configuration(TezosRuntimeConfiguration {
        debug_mode: false,
        log_enabled: false,
        no_of_ffi_calls_treshold_for_gc: 1000,
    })
    .unwrap()
    .unwrap();
}

fn sample_operations_decoded() -> Vec<Option<Vec<Operation>>> {
    vec![
        Some(vec![
            Operation::from_bytes(hex::decode(OPERATION).unwrap()).unwrap()
        ]),
        Some(vec![]),
        Some(vec![]),
        Some(vec![]),
    ]
}

fn apply_block_params_roundtrip(
    chain_id: RustBytes,
    block_header: BlockHeader,
    operations: Vec<Option<Vec<Operation>>>,
) -> Result<(), OcamlError> {
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let ref chain_id = to_ocaml!(gc, chain_id).keep(gc);
            let block_header = block_header.as_bytes().unwrap();
            let ref block_header = to_ocaml!(gc, block_header).keep(gc);
            let empty_vec = vec![];
            let operations: Vec<Vec<RustBytes>> = operations
                .iter()
                .map(|op| {
                    op.as_ref()
                        .unwrap_or(&empty_vec)
                        .iter()
                        .map(|op| op.as_bytes().unwrap())
                        .collect()
                })
                .collect();
            let operations: OCaml<OCamlList<OCamlList<OCamlBytes>>> = to_ocaml!(gc, operations);

            let result = ocaml_call!(tezos_ffi::apply_block_params_roundtrip(
                gc,
                gc.get(chain_id),
                gc.get(block_header),
                operations
            ))
            .unwrap();

            // Convert from OCaml
            let (_chain_id, block_header, operations): (RustBytes, RustBytes, Vec<Vec<RustBytes>>) =
                result.into_rust();

            // Decode result
            let _block_header: BlockHeader = BlockHeader::from_bytes(block_header).unwrap();
            let _operations: Vec<Vec<Operation>> = operations
                .iter()
                .map(|ops| {
                    ops.iter()
                        .map(|op| Operation::from_bytes(op).unwrap())
                        .collect()
                })
                .collect();

            ()
        })
    })
}

fn apply_block_params_decoded_roundtrip(
    chain_id: RustBytes,
    block_header: BlockHeader,
    operations: Vec<Option<Vec<Operation>>>,
) -> Result<(), OcamlError> {
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let ref chain_id = to_ocaml!(gc, chain_id).keep(gc);
            let ref block_header = to_ocaml!(gc, FfiBlockHeader(&block_header)).keep(gc);
            let empty_vec = vec![];
            let operations: Vec<Vec<_>> = operations
                .iter()
                .map(|op| {
                    op.as_ref()
                        .unwrap_or(&empty_vec)
                        .iter()
                        .map(|op| FfiOperation(op))
                        .collect()
                })
                .collect();
            let operations = to_ocaml!(gc, operations);

            let _result = ocaml_call!(tezos_ffi::apply_block_params_decoded_roundtrip(
                gc,
                gc.get(chain_id),
                gc.get(block_header),
                operations
            ))
            .unwrap();

            // Convert from OCaml
            //let (_chain_id, block_header, operations): (RustBytes, FfiBlockHeader, Vec<Vec<Operation>>) =
            //    result.into_rust();

            ()
        })
    })
}

fn criterion_benchmark(c: &mut Criterion) {
    init_bench_runtime();

    c.bench_function("apply_block_params_roundtrip", |b| {
        let chain_id = hex::decode(CHAIN_ID).unwrap();
        let block_header = hex::decode(HEADER).unwrap();
        let block_header_decoded = BlockHeader::from_bytes(block_header).unwrap();
        let operations_decoded = sample_operations_decoded();

        b.iter(|| {
            apply_block_params_roundtrip(
                black_box(chain_id.clone()),
                black_box(block_header_decoded.clone()),
                black_box(operations_decoded.clone()),
            )
        })
    });

    c.bench_function("apply_block_params_decoded_roundtrip", |b| {
        let chain_id = hex::decode(CHAIN_ID).unwrap();
        let block_header = hex::decode(HEADER).unwrap();
        let block_header_decoded = BlockHeader::from_bytes(block_header).unwrap();
        let operations_decoded = sample_operations_decoded();

        b.iter(|| {
            apply_block_params_decoded_roundtrip(
                black_box(chain_id.clone()),
                black_box(block_header_decoded.clone()),
                black_box(operations_decoded.clone()),
            )
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
