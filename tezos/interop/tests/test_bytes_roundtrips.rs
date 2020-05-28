// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use std::{env, thread};

use ocaml::{List, Tuple, Value};
use serial_test::serial;

use crypto::hash::HashType;
use tezos_api::ffi::{ApplyBlockRequest, ApplyBlockRequestBuilder, ApplyBlockResponse, FfiMessage, RustBytes, TezosRuntimeConfiguration};
use tezos_context::channel::{context_receive, ContextAction, enable_context_channel};
use tezos_interop::ffi;
use tezos_interop::ffi::{Interchange, OcamlBytes, OcamlHash};
use tezos_interop::runtime;
use tezos_interop::runtime::OcamlError;
use tezos_messages::p2p::binary_message::BinaryMessage;
use tezos_messages::p2p::encoding::prelude::*;

const CHAIN_ID: &str = "8eceda2f";
const CONTEXT_HASH: &str = "2f358bab4d28c4ee733ad7f2b01dcf116b33474b8c3a6cb40cccda2bdddd6d72";
const HEADER_HASH: &str = "61e687e852460b28f0f9540ccecf8f6cf87a5ad472c814612f0179caf4b9f673";
const HEADER: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";
const OPERATION: &str = "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a";
const OPERATION_HASH: &str = "7e73e3da041ea251037af062b7bc04b37a5ee38bc7e229e7e20737071ed73af4";
const MAX_OPERATIONS_TTL: i32 = 5;

fn init_test_runtime() {
    // init runtime and turn on/off ocaml logging
    ffi::change_runtime_configuration(
        TezosRuntimeConfiguration {
            debug_mode: false,
            log_enabled: is_ocaml_log_enabled(),
            no_of_ffi_calls_treshold_for_gc: no_of_ffi_calls_treshold_for_gc(),
        }
    ).unwrap().unwrap();
}

macro_rules! roundtrip_test {
        ($test_name:ident, $test_fn:expr, $counts:expr) => {
            #[test]
            #[serial]
            fn $test_name() {
                init_test_runtime();

                for i in 0..$counts {
                    let result = $test_fn(i);
                    if result.is_err() {
                        println!("$test_fn roundtrip number {} failed!", i);
                    }
                    assert!(result.is_ok())
                }
            }
        };
}

// fn bytes_encoding_roundtrip<MSG: FfiMessage + 'static>(orig_data: MSG, encoding: &'static Encoding, ffi_fn_name: String) -> Result<(), OcamlError> {
fn bytes_encoding_roundtrip<MSG: FfiMessage + 'static>(orig_data: MSG, ffi_fn_name: String) -> Result<(), OcamlError> {
    runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value(&ffi_fn_name).expect(&format!("function '{}' is not registered", &ffi_fn_name));

        let data_as_rust_bytes: RustBytes = orig_data.as_rust_bytes().expect("failed to convert");

        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<OcamlBytes>(
            data_as_rust_bytes.convert_to(),
        );

        // check
        let result: OcamlBytes = result.expect("failed to call ffi").into();
        let result: RustBytes = result.convert_to();

        let result = MSG::from_rust_bytes(result).expect("failed to convert in ffi");
        assert_eq!(orig_data, result);

        ()
    })
}

#[test]
#[serial]
fn test_chain_id_roundtrip_one() {
    init_test_runtime();

    assert!(test_chain_id_roundtrip(1).is_ok())
}

fn test_chain_id_roundtrip(iteration: i32) -> Result<(), failure::Error> {
    let chain_id: RustBytes = hex::decode(CHAIN_ID).unwrap();

    let result = runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("chain_id_roundtrip").expect("function 'chain_id_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<OcamlHash>(chain_id.convert_to());
        let result: OcamlHash = result.unwrap().into();
        assert_eq_hash(CHAIN_ID, result);
        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_chain_id_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

#[test]
#[serial]
fn test_block_header_roundtrip_one() {
    init_test_runtime();

    assert!(test_block_header_roundtrip(1).is_ok())
}

fn test_block_header_roundtrip(iteration: i32) -> Result<(), failure::Error> {
    let header: RustBytes = hex::decode(HEADER).unwrap();

    let result = runtime::execute(move || {

        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("block_header_roundtrip").expect("function 'block_header_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<OcamlBytes>(header.convert_to());
        let result: Tuple = result.unwrap().into();
        assert_eq_hash_and_header(HEADER_HASH, HEADER, result);
        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_block_header_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

#[test]
#[serial]
fn test_block_header_with_hash_roundtrip_one() {
    init_test_runtime();

    assert!(test_block_header_with_hash_roundtrip(1).is_ok())
}

fn test_block_header_with_hash_roundtrip(iteration: i32) -> Result<(), failure::Error> {
    let header_hash: RustBytes = hex::decode(HEADER_HASH).unwrap();
    let header: RustBytes = hex::decode(HEADER).unwrap();

    let result = runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("block_header_with_hash_roundtrip").expect("function 'block_header_with_hash_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call2_exn::<OcamlHash, OcamlBytes>(
            header_hash.convert_to(),
            header.convert_to(),
        );
        let result: Tuple = result.unwrap().into();
        assert_eq_hash_and_header(HEADER_HASH, HEADER, result);
        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_block_header_with_hash_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

#[test]
#[serial]
fn test_operation_roundtrip_one() {
    init_test_runtime();

    assert!(test_operation_roundtrip(1).is_ok())
}

fn test_operation_roundtrip(iteration: i32) -> Result<(), failure::Error> {
    let operation: RustBytes = hex::decode(OPERATION).unwrap();

    let result = runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("operation_roundtrip").expect("function 'operation_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<OcamlBytes>(operation.convert_to());

        // check
        let result: OcamlBytes = result.unwrap().into();
        assert_eq!(OPERATION, hex::encode(result.convert_to()).as_str());

        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_operation_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

#[test]
#[serial]
fn test_operations_list_list_roundtrip_one() {
    init_test_runtime();

    assert!(test_operations_list_list_roundtrip(1, sample_operations(), 4, 1).is_ok());
    assert!(test_operations_list_list_roundtrip(1, sample_operations2(), 4, 0).is_ok());
    assert!(test_operations_list_list_roundtrip(1, sample_operations3(), 5, 1).is_ok());
}

fn test_operations_list_list_roundtrip_for_bench(iteration: i32) -> Result<(), failure::Error> {
    Ok(assert!(test_operations_list_list_roundtrip(iteration, sample_operations(), 4, 1).is_ok()))
}

fn test_operations_list_list_roundtrip(iteration: i32, operations: Vec<Option<Vec<RustBytes>>>, expected_list_count: usize, expected_list_0_count: usize) -> Result<(), failure::Error> {
    let result = runtime::execute(move || {
        let operations_list_list_ocaml = ffi::operations_to_ocaml(&operations);

        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("operations_list_list_roundtrip").expect("function 'operations_list_list_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<List>(operations_list_list_ocaml);

        // check
        assert_eq_operations(List::from(result.unwrap()), expected_list_count, expected_list_0_count);

        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_operations_list_list_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

#[test]
#[serial]
fn test_operations_list_list_roundtrip_times() {
    init_test_runtime();

    for i in 0..10000 {
        let result = test_operations_list_list_roundtrip(i, sample_operations(), 4, 1);
        if result.is_err() {
            println!("test_operations_list_list_roundtrip_times roundtrip number {} failed!", i);
        }
        assert!(result.is_ok())
    }
}

roundtrip_test!(test_apply_block_params_roundtrip_calls, test_apply_block_params_roundtrip, 10000);

fn test_apply_block_params_roundtrip(iteration: i32) -> Result<(), failure::Error> {
    let chain_id = hex::decode(CHAIN_ID).unwrap();
    let block_header = hex::decode(HEADER).unwrap();
    let operations = sample_operations();

    Ok(
        assert!(
            apply_block_params_roundtrip(chain_id, block_header, operations).is_ok(),
            format!("test_apply_block_params_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

fn apply_block_params_roundtrip(chain_id: RustBytes,
                                block_header: RustBytes,
                                operations: Vec<Option<Vec<RustBytes>>>) -> Result<(), OcamlError> {
    runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("apply_block_params_roundtrip").expect("function 'apply_block_params_roundtrip' is not registered");

        let result: Result<Value, ocaml::Error> = roundtrip.call3_exn::<OcamlHash, OcamlBytes, List>(
            chain_id.convert_to(),
            block_header.convert_to(),
            ffi::operations_to_ocaml(&operations),
        );

        // check
        let result: Tuple = result.unwrap().into();
        assert_eq!(3, result.len());

        // check chain_id
        assert_eq_hash(CHAIN_ID, result.get(0).unwrap().into());

        // check header
        assert_eq_bytes(HEADER, result.get(1).unwrap().into());

        // check operations
        assert_eq_operations(List::from(result.get(2).unwrap()), 4, 1);

        ()
    })
}

roundtrip_test!(test_apply_block_request_roundtrip_calls, test_apply_block_request_roundtrip, 10000);

fn test_apply_block_request_roundtrip(iteration: i32) -> Result<(), failure::Error> {
    // request
    let request: ApplyBlockRequest = ApplyBlockRequestBuilder::default()
        .chain_id(hex::decode(CHAIN_ID).unwrap())
        .block_header(BlockHeader::from_bytes(hex::decode(HEADER).unwrap()).unwrap())
        .pred_header(BlockHeader::from_bytes(hex::decode(HEADER).unwrap()).unwrap())
        .max_operations_ttl(MAX_OPERATIONS_TTL)
        .operations(ApplyBlockRequest::convert_operations(&block_operations_from_hex(HEADER_HASH, sample_operations_for_request())))
        .build().unwrap();

    Ok(
        assert!(
            bytes_encoding_roundtrip(request, String::from("apply_block_request_roundtrip")).is_ok(),
            format!("test_apply_block_params_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

roundtrip_test!(test_apply_block_response_roundtrip_calls, test_apply_block_response_roundtrip, 10000);

fn test_apply_block_response_roundtrip(iteration: i32) -> Result<(), failure::Error> {
    // request
    let response: ApplyBlockResponse = ApplyBlockResponse {
        validation_result_message: "validation_result_message".to_string(),
        context_hash: HashType::ContextHash.string_to_bytes("CoV16kW8WgL51SpcftQKdeqc94D6ekghMgPMmEn7TSZzFA697PeE").expect("failed to convert"),
        block_header_proto_json: "block_header_proto_json".to_string(),
        block_header_proto_metadata_json: "block_header_proto_metadata_json".to_string(),
        operations_proto_metadata_json: "operations_proto_metadata_json".to_string(),
        max_operations_ttl: 6,
        last_allowed_fork_level: 8,
        forking_testchain: true,
        forking_testchain_data: None,
    };

    Ok(
        assert!(
            bytes_encoding_roundtrip(response, String::from("apply_block_response_roundtrip")).is_ok(),
            format!("test_apply_block_response_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

#[test]
#[serial]
fn test_context_callback() {
    init_test_runtime();

    let expected_count = 10000;
    let expected_context_hash = hex::decode(CONTEXT_HASH).unwrap();
    let expected_header_hash = hex::decode(HEADER_HASH).unwrap();
    let expected_operation_hash = hex::decode(OPERATION_HASH).unwrap();
    let expected_key: Vec<String> = vec!["data".to_string(), "contracts".to_string(), "contract".to_string(), "amount".to_string()];
    let expected_data = hex::decode(HEADER).unwrap();

    let expected_context_hash_cloned = expected_context_hash.clone();
    let expected_header_hash_cloned = expected_header_hash.clone();
    let expected_operation_hash_cloned = expected_operation_hash.clone();
    let expected_key_cloned = expected_key.clone();
    let expected_data_cloned = expected_data.clone();

    enable_context_channel();
    let handle = thread::spawn(move || {
        let mut received = 0;
        for _ in 0..expected_count {
            let action = context_receive().unwrap();
            received += 1;

            match action {
                ContextAction::Set {
                    context_hash,
                    block_hash,
                    operation_hash,
                    key,
                    value,
                    ..
                } => {
                    assert!(context_hash.is_some());
                    assert_eq!(expected_context_hash_cloned, context_hash.unwrap());

                    assert!(block_hash.is_some());
                    assert_eq!(expected_header_hash_cloned, block_hash.unwrap());

                    assert!(operation_hash.is_some());
                    assert_eq!(expected_operation_hash_cloned, operation_hash.unwrap());

                    assert_eq!(expected_key_cloned.clone(), key);
                    assert_eq!(expected_data_cloned.clone(), value);
                }
                ContextAction::Checkout {
                    context_hash,
                    ..
                } => {
                    assert_eq!(expected_context_hash_cloned, context_hash);
                }
                _ => panic!("test failed - waiting just 'Set' action!")
            }
        }
        received
    });

    call_to_send_context_events(
        expected_count,
        expected_context_hash,
        expected_header_hash,
        expected_operation_hash,
        expected_key,
        expected_data,
    );

    let received = handle.join().unwrap();
    assert_eq!(expected_count, received)
}

fn call_to_send_context_events(
    count: i32,
    context_hash: RustBytes,
    block_header_hash: RustBytes,
    operation_hash: RustBytes,
    key: Vec<String>,
    data: RustBytes) {
    runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("context_callback_roundtrip").expect("function 'context_callback_roundtrip' is not registered");

        let context_hash: OcamlHash = context_hash.convert_to();
        let block_header_hash: OcamlHash = block_header_hash.convert_to();
        let operation_hash: OcamlHash = operation_hash.convert_to();
        let data: OcamlBytes = data.convert_to();

        let result: Result<Value, ocaml::Error> = roundtrip.call_n_exn(
            [
                Value::i32(count),
                Value::from(context_hash),
                Value::from(block_header_hash),
                Value::from(operation_hash),
                Value::from(List::from(key.as_slice())),
                Value::from(data)
            ]
        );

        // check
        assert!(result.is_ok());

        ()
    }).unwrap()
}

fn block_operations_from_hex(block_hash: &str, hex_operations: Vec<Vec<RustBytes>>) -> Vec<Option<OperationsForBlocksMessage>> {
    hex_operations
        .into_iter()
        .map(|bo| {
            let ops = bo
                .into_iter()
                .map(|op| Operation::from_bytes(op).unwrap())
                .collect();
            Some(OperationsForBlocksMessage::new(OperationsForBlock::new(hex::decode(block_hash).unwrap(), 4), Path::Op, ops))
        })
        .collect()
}

fn sample_operations() -> Vec<Option<Vec<RustBytes>>> {
    vec![
        Some(vec![hex::decode(OPERATION).unwrap()]),
        Some(vec![]),
        Some(vec![]),
        Some(vec![])
    ]
}

fn sample_operations_for_request() -> Vec<Vec<RustBytes>> {
    vec![
        vec![hex::decode(OPERATION).unwrap()],
        vec![],
        vec![],
        vec![hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08").unwrap()],
        vec![]
    ]
}

fn sample_operations2() -> Vec<Option<Vec<RustBytes>>> {
    vec![
        Some(vec![]),
        Some(vec![]),
        Some(vec![]),
        Some(vec![])
    ]
}

fn sample_operations3() -> Vec<Option<Vec<RustBytes>>> {
    vec![
        Some(vec![hex::decode(OPERATION).unwrap()]),
        Some(vec![]),
        Some(vec![]),
        Some(vec![hex::decode("10490b79070cf19175cd7e3b9c1ee66f6e85799980404b119132ea7e58a4a97e000008c387fa065a181d45d47a9b78ddc77e92a881779ff2cbabbf9646eade4bf1405a08e00b725ed849eea46953b10b5cdebc518e6fd47e69b82d2ca18c4cf6d2f312dd08").unwrap()]),
        Some(vec![])
    ]
}

fn assert_eq_bytes(expected: &str, bytes: OcamlBytes) {
    assert!(!bytes.is_empty());
    let bytes: RustBytes = bytes.convert_to();
    let bytes_ocaml = hex::encode(bytes);
    assert_eq!(expected, bytes_ocaml.as_str());
}

fn assert_eq_hash(expected: &str, hash: OcamlHash) {
    assert!(!hash.is_empty());
    let hash: RustBytes = hash.convert_to();
    let hash_ocaml = hex::encode(hash);
    assert_eq!(expected, hash_ocaml.as_str());
}

fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>().unwrap()
}

fn no_of_ffi_calls_treshold_for_gc() -> i32 {
    env::var("OCAML_CALLS_GC")
        .unwrap_or("1000".to_string())
        .parse::<i32>().unwrap()
}

fn assert_eq_hash_and_header(expected_hash: &str, expected_header: &str, header_tuple: Tuple) {
    assert_eq!(2, header_tuple.len());
    assert_eq_hash(expected_hash, header_tuple.get(0).unwrap().into());
    assert_eq_bytes(expected_header, header_tuple.get(1).unwrap().into());
}

fn assert_eq_operations(result: List, expected_list_count: usize, expected_list_0_count: usize) {
    assert_eq!(expected_list_count, result.len());

    let operations_list_0: List = result.hd().unwrap().into();
    assert_eq!(expected_list_0_count, operations_list_0.len());

    if expected_list_0_count > 0 {
        let operation_0_0: OcamlBytes = operations_list_0.hd().unwrap().into();
        assert_eq!(OPERATION, hex::encode(operation_0_0.convert_to()).as_str());
    }
}

// run as: cargo bench --tests --nocapture
mod benches {
    use test::Bencher;

    use crate::*;

    macro_rules! bench_test {
        ($test_name:ident, $f:expr) => {
            #[bench]
            fn $test_name(b: &mut Bencher) {
                init_test_runtime();

                let mut counter = 0;
                let mut counter_failed = 0;
                b.iter(|| {
                    counter += 1;
                    let result: Result<(), failure::Error> = $f(counter);
                    if let Err(_) = result {
                        counter_failed += 1;
                    }
                });
                assert_eq!(0, counter_failed);
            }
        };
    }

    bench_test!(bench_test_chain_id_roundtrip, test_chain_id_roundtrip);
    bench_test!(bench_test_block_header_roundtrip, test_block_header_roundtrip);
    bench_test!(bench_test_block_header_with_hash_roundtrip, test_block_header_with_hash_roundtrip);
    bench_test!(bench_test_operations_list_list_roundtrip_one, test_operations_list_list_roundtrip_for_bench);
    bench_test!(bench_test_apply_block_params_roundtrip_one, test_apply_block_params_roundtrip);
    bench_test!(bench_test_apply_block_request_roundtrip_one, test_apply_block_request_roundtrip);
    bench_test!(bench_test_apply_block_response_roundtrip_one, test_apply_block_response_roundtrip);
}
