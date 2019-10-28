// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::env;

use ocaml::{Error, List, Tuple, Value};

use tezos_api::ffi::{RustBytes, TezosRuntimeConfiguration};
use tezos_interop::ffi;
use tezos_interop::ffi::{Interchange, OcamlBytes};
use tezos_interop::runtime;

const HEADER_HASH: &str = "60ab6d8d2a6b1c7a391f00aa6c1fc887eb53797214616fd2ce1b9342ad4965a4";
const HEADER: &str = "0000000201dd9fb5edc4f29e7d28f41fe56d57ad172b7686ed140ad50294488b68de29474d000000005c017cd804683625c2445a4e9564bf710c5528fd99a7d150d2a2a323bc22ff9e2710da4f6d0000001100000001000000000800000000000000029bd8c75dec93c276d2d8e8febc3aa6c9471cb2cb42236b3ab4ca5f1f2a0892f6000500000003ba671eef00d6a8bea20a4677fae51268ab6be7bd8cfc373cd6ac9e0a00064efcc404e1fb39409c5df255f7651e3d1bb5d91cb2172b687e5d56ebde58cfd92e1855aaafbf05";
const OPERATION: &str = "a14f19e0df37d7b71312523305d71ac79e3d989c1c1d4e8e884b6857e4ec1627000000000236663bacdca76094fdb73150092659d463fec94eda44ba4db10973a1ad057ef53a5b3239a1b9c383af803fc275465bd28057d68f3cab46adfd5b2452e863ff0a";

fn sample_operations() -> Vec<Option<Vec<RustBytes>>> {
    vec![
        Some(vec![hex::decode(OPERATION).unwrap()]),
        Some(vec![]),
        Some(vec![]),
        Some(vec![])
    ]
}

#[test]
fn test_block_header_roundtrip() {
    ffi::change_runtime_configuration(TezosRuntimeConfiguration { log_enabled: is_ocaml_log_enabled() }).unwrap().unwrap();

    let header_hash = hex::decode(HEADER_HASH).unwrap();
    let header = hex::decode(HEADER).unwrap();

    let result = runtime::execute(move || {
        // prepare header input
        let header_tuple = ffi::block_header_to_ocaml(&header_hash, &header).unwrap();

        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("block_header_roundtrip").expect("function 'block_header_roundtrip' is not registered");
        let result: Result<Value, Error> = roundtrip.call_exn::<Value>(header_tuple.into());
        let result: Tuple = result.unwrap().into();
        assert_eq!(2, result.len());

        // ocaml result to bytes
        let header_hash_ocaml: OcamlBytes = result.get(0).unwrap().into();
        let header_hash_ocaml = hex::encode(header_hash_ocaml.data());
        assert_eq!(HEADER_HASH, header_hash_ocaml.as_str());

        let header_ocaml: OcamlBytes = result.get(1).unwrap().into();
        let header_ocaml = hex::encode(header_ocaml.data());
        assert_eq!(HEADER, header_ocaml.as_str());

        ()
    });

    assert!(result.is_ok())
}

#[test]
fn test_operation_roundtrip() {
    ffi::change_runtime_configuration(TezosRuntimeConfiguration { log_enabled: is_ocaml_log_enabled() }).unwrap().unwrap();

    let operation: RustBytes = hex::decode(OPERATION).unwrap();

    let result = runtime::execute(move || {
        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("operation_roundtrip").expect("function 'operation_roundtrip' is not registered");
        let result: Result<Value, Error> = roundtrip.call_exn::<OcamlBytes>(operation.convert_to());

        // check
        let result: OcamlBytes = result.unwrap().into();
        assert_eq!(OPERATION, hex::encode(result.data()).as_str());

        ()
    });

    assert!(result.is_ok())
}


#[test]
fn test_operations_list_list_roundtrip() {
    ffi::change_runtime_configuration(TezosRuntimeConfiguration { log_enabled: is_ocaml_log_enabled() }).unwrap().unwrap();

    let result = runtime::execute(move || {
        let operations_list_list_ocaml = ffi::operations_to_ocaml(&sample_operations());

        // sent bytes to ocaml
        let roundtrip = ocaml::named_value("operations_list_list_roundtrip").expect("function 'operations_list_list_roundtrip' is not registered");
        let result: Result<Value, Error> = roundtrip.call_exn::<List>(operations_list_list_ocaml);

        // check
        let result: List = result.unwrap().into();
        assert_eq!(4, result.len());

        let operations_list_0: List = result.hd().unwrap().into();
        assert_eq!(1, operations_list_0.len());

        let operation_0_0: OcamlBytes = operations_list_0.hd().unwrap().into();
        assert_eq!(OPERATION, hex::encode(operation_0_0.data()).as_str());

        ()
    });

    assert!(result.is_ok())
}

fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>().unwrap()
}