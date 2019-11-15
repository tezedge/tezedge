// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

use std::env;

use ocaml::{Array, List, Tuple, Value};

use tezos_api::ffi::{RustBytes, TezosRuntimeConfiguration};
use tezos_interop::ffi;
use tezos_interop::ffi::{Interchange, OcamlBytes};
use tezos_interop::runtime;
use tezos_interop::runtime::OcamlError;

const CHAIN_ID: &str = "8eceda2f";
const HEADER_HASH: &str = "61e687e852460b28f0f9540ccecf8f6cf87a5ad472c814612f0179caf4b9f673";
const HEADER: &str = "0000000301a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000005c017ed604dfcb6b41e91650bb908618b2740a6167d9072c3230e388b24feeef04c98dc27f000000110000000100000000080000000000000005f06879947f3d9959090f27054062ed23dbf9f7bd4b3c8a6e86008daabb07913e000c00000003e5445371002b9745d767d7f164a39e7f373a0f25166794cba491010ab92b0e281b570057efc78120758ff26a33301870f361d780594911549bcb7debbacd8a142e0b76a605";
const OPERATION: &str = "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a";

macro_rules! tezos_test {
    ($f:expr) => ( (stringify!($f), $f) )
}

#[test]
fn run_tests() {
    // init runtime and turn on/off ocaml logging
    ffi::change_runtime_configuration(TezosRuntimeConfiguration::new(is_ocaml_log_enabled(), no_of_ffi_calls_treshold_for_gc())).unwrap().unwrap();

    // We cannot run tests in parallel, because tezos does not handle situation when multiple storage
    // directories are initialized
    let tests: [(&str, fn() -> Result<(), failure::Error>); 9] = [
        tezos_test!(test_chain_id_roundtrip_one),
        tezos_test!(test_block_header_roundtrip_one),
        tezos_test!(test_block_header_with_hash_roundtrip_one),
        tezos_test!(test_operation_roundtrip_one),
        tezos_test!(test_operations_array_array_roundtrip_one),
        tezos_test!(test_operations_list_list_roundtrip_one),
        tezos_test!(test_operations_list_list_roundtrip_times),
        tezos_test!(test_apply_block_params_roundtrip_one),
        tezos_test!(test_apply_block_params_roundtrip_times),
    ];

    for (name, f) in tests.iter() {
        let result = f();
        assert!(result.is_ok(), "Tezos roundtrip test {:?} failed with error: {:?}", name, &result);
    }
}

fn test_chain_id_roundtrip_one() -> Result<(), failure::Error> {
    test_chain_id_roundtrip(1)
}

fn test_chain_id_roundtrip(iteration: i32) -> Result<(), failure::Error> {

    let chain_id: RustBytes = hex::decode(CHAIN_ID).unwrap();

    let result = runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("chain_id_roundtrip").expect("function 'chain_id_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<OcamlBytes>(chain_id.convert_to());
        let result: OcamlBytes = result.unwrap().into();
        assert_eq_bytes(CHAIN_ID, result);
        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_chain_id_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

fn test_block_header_roundtrip_one() -> Result<(), failure::Error> {
    test_block_header_roundtrip(1)
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

fn test_block_header_with_hash_roundtrip_one() -> Result<(), failure::Error> {
    test_block_header_with_hash_roundtrip(1)
}

fn test_block_header_with_hash_roundtrip(iteration: i32) -> Result<(), failure::Error> {

    let header_hash: RustBytes = hex::decode(HEADER_HASH).unwrap();
    let header: RustBytes = hex::decode(HEADER).unwrap();

    let result = runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("block_header_with_hash_roundtrip").expect("function 'block_header_with_hash_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call2_exn::<OcamlBytes, OcamlBytes>(
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

fn test_operation_roundtrip_one() -> Result<(), failure::Error> {
    test_operation_roundtrip(1)
}

fn test_operation_roundtrip(iteration: i32) -> Result<(), failure::Error> {

    let operation: RustBytes = hex::decode(OPERATION).unwrap();

    let result = runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("operation_roundtrip").expect("function 'operation_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<OcamlBytes>(operation.convert_to());

        // check
        let result: OcamlBytes = result.unwrap().into();
        assert_eq!(OPERATION, hex::encode(result.data()).as_str());

        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_operation_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

fn test_operations_list_list_roundtrip_one() -> Result<(), failure::Error> {
    test_operations_list_list_roundtrip(1)
}

fn test_operations_list_list_roundtrip(iteration: i32) -> Result<(), failure::Error> {

    let result = runtime::execute(move || {
        let operations_list_list_ocaml = ffi::operations_to_ocaml(&sample_operations());

        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("operations_list_list_roundtrip").expect("function 'operations_list_list_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<List>(operations_list_list_ocaml);

        // check
        assert_eq_operations(List::from(result.unwrap()));

        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_operations_list_list_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

fn test_operations_array_array_roundtrip_one() -> Result<(), failure::Error> {
    test_operations_array_array_roundtrip(1)
}

fn test_operations_array_array_roundtrip(iteration: i32) -> Result<(), failure::Error> {

    let result = runtime::execute(move || {
        let operations_list_list_ocaml = operations_to_ocaml_array(&sample_operations());

        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("operations_array_array_roundtrip").expect("function 'operations_array_array_roundtrip' is not registered");
        let result: Result<Value, ocaml::Error> = roundtrip.call_exn::<Array>(operations_list_list_ocaml);

        // check
        assert_eq_operations_as_array(Array::from(result.unwrap()));

        ()
    });

    Ok(
        assert!(
            result.is_ok(),
            format!("test_operations_array_array_roundtrip roundtrip iteration: {} failed!", iteration)
        )
    )
}

fn test_operations_list_list_roundtrip_times() -> Result<(), failure::Error> {

    for i in 0..10000 {
        let result = test_operations_list_list_roundtrip(i);
        if result.is_err() {
            println!("test_operations_list_list_roundtrip_times roundtrip number {} failed!", i);
        }
        assert!(result.is_ok())
    }

    Ok(())
}

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

fn test_apply_block_params_roundtrip_one() -> Result<(), failure::Error> {
    test_apply_block_params_roundtrip(1)
}

fn test_apply_block_params_roundtrip_times() -> Result<(), failure::Error> {

    for i in 0..10000 {
        let result = test_apply_block_params_roundtrip(i);
        if result.is_err() {
            println!("apply_block_params_roundtrip_times roundtrip number {} failed!", i);
        }
        assert!(result.is_ok())
    }

    Ok(())
}

fn apply_block_params_roundtrip(chain_id: RustBytes,
                                block_header: RustBytes,
                                operations: Vec<Option<Vec<RustBytes>>>) -> Result<(), OcamlError> {
    runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("apply_block_params_roundtrip").expect("function 'apply_block_params_roundtrip' is not registered");

        let result: Result<Value, ocaml::Error> = roundtrip.call3_exn::<OcamlBytes, OcamlBytes, List>(
            chain_id.convert_to(),
            block_header.convert_to(),
            ffi::operations_to_ocaml(&operations),
        );

        // check
        let result: Tuple = result.unwrap().into();
        assert_eq!(3, result.len());

        // check chain_id
        assert_eq_bytes(CHAIN_ID, result.get(0).unwrap().into());

        // check header
        assert_eq_bytes(HEADER, result.get(1).unwrap().into());

        // check operations
        assert_eq_operations(List::from(result.get(2).unwrap()));

        ()
    })
}

fn sample_operations() -> Vec<Option<Vec<RustBytes>>> {
    vec![
        Some(vec![hex::decode(OPERATION).unwrap()]),
        Some(vec![]),
        Some(vec![]),
        Some(vec![])
    ]
}

fn assert_eq_bytes(expected_header: &str, header: OcamlBytes) {
    assert!(!header.is_empty());
    let header: RustBytes = header.convert_to();
    let header_ocaml = hex::encode(header);
    assert_eq!(expected_header, header_ocaml.as_str());
}

fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>().unwrap()
}

fn no_of_ffi_calls_treshold_for_gc() -> i32 {
    env::var("OCAML_CALLS_GC")
        .unwrap_or("2000".to_string())
        .parse::<i32>().unwrap()
}

fn assert_eq_hash_and_header(expected_hash: &str, expected_header: &str, header_tuple: Tuple) {
    assert_eq!(2, header_tuple.len());

    // ocaml result to bytes
    let header_hash_ocaml: OcamlBytes = header_tuple.get(0).unwrap().into();
    let header_hash_ocaml = hex::encode(header_hash_ocaml.data());
    assert_eq!(expected_hash, header_hash_ocaml.as_str());

    assert_eq_bytes(expected_header, header_tuple.get(1).unwrap().into());
}

fn assert_eq_operations(result: List) {
    assert_eq!(4, result.len());

    let operations_list_0: List = result.hd().unwrap().into();
    assert_eq!(1, operations_list_0.len());

    let operation_0_0: OcamlBytes = operations_list_0.hd().unwrap().into();
    assert_eq!(OPERATION, hex::encode(operation_0_0.data()).as_str());
}

fn assert_eq_operations_as_array(result: Array) {
    assert_eq!(4, result.len());

    let operations_list_0: Array = result.get(0).unwrap().into();
    assert_eq!(1, operations_list_0.len());

    let operation_0_0: OcamlBytes = operations_list_0.get(0).unwrap().into();
    assert_eq!(OPERATION, hex::encode(operation_0_0.data()).as_str());
}

fn operations_to_ocaml_array(operations: &Vec<Option<Vec<RustBytes>>>) -> Array {
    let mut operations_for_ocaml = Array::new(operations.len());

    operations
        .into_iter()
        .enumerate()
        .for_each(|(idx1, ops_option)| {
            let ops_array = if let Some(ops) = ops_option {
                let mut ops_array = Array::new(ops.len());
                ops
                    .into_iter()
                    .enumerate()
                    .for_each(|(idx2, op)| {
                        ops_array.set(idx2, Value::from(op.convert_to())).unwrap();
                    });
                ops_array
            } else {
                Array::new(0)
            };
            operations_for_ocaml.set(idx1, ops_array.into()).unwrap();
        });

    operations_for_ocaml
}

// run as: cargo bench --tests --nocapture
mod benches {
    use test::Bencher;

    use tezos_api::ffi::TezosRuntimeConfiguration;
    use tezos_interop::ffi;

    use crate::*;

    macro_rules! bench_test {
        ($test_name:ident, $f:expr) => {
            #[bench]
            fn $test_name(b: &mut Bencher) {
                ffi::change_runtime_configuration(TezosRuntimeConfiguration::new(is_ocaml_log_enabled(), no_of_ffi_calls_treshold_for_gc())).unwrap().unwrap();

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
    bench_test!(bench_test_operations_array_array_roundtrip, test_operations_array_array_roundtrip);
    bench_test!(bench_test_operations_list_list_roundtrip_one, test_operations_list_list_roundtrip);
    bench_test!(bench_test_apply_block_params_roundtrip_one, test_apply_block_params_roundtrip);
}