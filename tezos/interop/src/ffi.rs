// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml::{Array1, List, Str, Tuple, Value};
use serde_json;

use tezos_api::ffi::*;
use tezos_api::identity::Identity;

use crate::runtime;
use crate::runtime::OcamlError;

pub type OcamlBytes = Array1<u8>;
pub type OcamlHash = Str;

pub trait Interchange<T> {
    fn convert_to(&self) -> T;
    fn is_empty(&self) -> bool;
}

impl Interchange<OcamlBytes> for RustBytes {
    fn convert_to(&self) -> OcamlBytes {
        Array1::from(self.as_slice())
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl Interchange<RustBytes> for OcamlBytes {
    fn convert_to(&self) -> RustBytes {
        self.data().to_vec()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl Interchange<OcamlHash> for RustBytes {
    fn convert_to(&self) -> OcamlHash {
        Str::from(hex::encode(self).as_str())
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

impl Interchange<RustBytes> for OcamlHash {
    fn convert_to(&self) -> RustBytes {
        hex::decode(self.as_str()).unwrap()
    }

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

pub fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<Result<(), TezosRuntimeConfigurationError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("change_runtime_configuration").expect("function 'change_runtime_configuration' is not registered");
        match ocaml_function.call2_exn::<Value, Value>(
            Value::bool(settings.log_enabled),
            Value::i32(settings.no_of_ffi_calls_treshold_for_gc),
        ) {
            Ok(_) => {
                Ok(())
            }
            Err(e) => {
                Err(TezosRuntimeConfigurationError::from(e))
            }
        }
    })
}

pub fn init_storage(storage_data_dir: String, genesis: &'static GenesisChain, protocol_overrides: &'static ProtocolOverrides, enable_testchain: bool)
                    -> Result<Result<OcamlStorageInitInfo, TezosStorageInitError>, OcamlError> {
    runtime::execute(move || {
        // genesis configuration
        let mut genesis_tuple: Tuple = Tuple::new(3);
        genesis_tuple.set(0, Str::from(genesis.time.as_str()).into()).unwrap();
        genesis_tuple.set(1, Str::from(genesis.block.as_str()).into()).unwrap();
        genesis_tuple.set(2, Str::from(genesis.protocol.as_str()).into()).unwrap();

        // protocol overrides
        let protocol_overrides_tuple: Tuple = protocol_overrides_to_ocaml(protocol_overrides)?;

        let ocaml_function = ocaml::named_value("init_storage").expect("function 'init_storage' is not registered");
        match ocaml_function.call_n_exn(
            [
                Value::from(Str::from(storage_data_dir.as_str())),
                Value::from(genesis_tuple),
                Value::from(protocol_overrides_tuple),
                Value::bool(enable_testchain)
            ]
        ) {
            Ok(result) => {
                let ocaml_result: Tuple = result.into();

                // expecting 3 tuples
                // 1. main and test chain
                let chains: Tuple = ocaml_result.get(0).unwrap().into();
                let main_chain_id: OcamlHash = chains.get(0).unwrap().into();

                let test_chain: Tuple = chains.get(1).unwrap().into();
                let test_chain_id: OcamlHash = test_chain.get(0).unwrap().into();
                let test_chain: Option<TestChain> = if test_chain_id.is_empty() {
                    None
                } else {
                    let protocol: OcamlHash = test_chain.get(1).unwrap().into();
                    let time: Str = test_chain.get(2).unwrap().into();
                    Some(TestChain {
                        chain_id: test_chain_id.convert_to(),
                        protocol_hash: protocol.convert_to(),
                        expiration_date: String::from(time.as_str()),
                    })
                };

                // 2. genesis and current head
                let headers: Tuple = ocaml_result.get(1).unwrap().into();
                let genesis_block_header_hash: OcamlHash = headers.get(0).unwrap().into();
                let genesis_block_header: OcamlBytes = headers.get(1).unwrap().into();
                let current_block_header_hash: OcamlHash = headers.get(2).unwrap().into();

                // 3. list known protocols
                let supported_protocol_hashes: List = ocaml_result.get(2).unwrap().into();
                let supported_protocol_hashes: Vec<RustBytes> = supported_protocol_hashes.to_vec()
                    .iter()
                    .map(|protocol_hash| {
                        let protocol_hash: OcamlBytes = protocol_hash.clone().into();
                        protocol_hash.convert_to()
                    })
                    .collect();

                Ok(OcamlStorageInitInfo {
                    chain_id: main_chain_id.convert_to(),
                    test_chain,
                    genesis_block_header_hash: genesis_block_header_hash.convert_to(),
                    genesis_block_header: genesis_block_header.convert_to(),
                    current_block_header_hash: current_block_header_hash.convert_to(),
                    supported_protocol_hashes,
                })
            }
            Err(e) => {
                Err(TezosStorageInitError::from(e))
            }
        }
    })
}

pub fn get_current_block_header(chain_id: RustBytes) -> Result<Result<RustBytes, BlockHeaderError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_current_block_header").expect("function 'get_current_block_header' is not registered");
        match ocaml_function.call_exn::<OcamlHash>(chain_id.convert_to()) {
            Ok(block_header) => {
                let block_header: OcamlBytes = block_header.into();
                if block_header.is_empty() {
                    Err(BlockHeaderError::ExpectedButNotFound)
                } else {
                    Ok(block_header.convert_to())
                }
            }
            Err(e) => {
                Err(BlockHeaderError::from(e))
            }
        }
    })
}

pub fn get_block_header(chain_id: RustBytes, block_header_hash: RustBytes) -> Result<Result<Option<RustBytes>, BlockHeaderError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("get_block_header").expect("function 'get_block_header' is not registered");
        match ocaml_function.call2_exn::<OcamlHash, OcamlHash>(chain_id.convert_to(), block_header_hash.convert_to()) {
            Ok(block_header) => {
                let block_header: OcamlBytes = block_header.into();
                if block_header.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(block_header.convert_to()))
                }
            }
            Err(e) => {
                Err(BlockHeaderError::from(e))
            }
        }
    })
}

pub fn apply_block(
    chain_id: RustBytes,
    block_header: RustBytes,
    operations: Vec<Option<Vec<RustBytes>>>)
    -> Result<Result<ApplyBlockResult, ApplyBlockError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("apply_block").expect("function 'apply_block' is not registered");

        // call ffi
        match ocaml_function.call3_exn::<OcamlHash, OcamlBytes, List>(
            chain_id.convert_to(),
            block_header.convert_to(),
            operations_to_ocaml(&operations),
        ) {
            Ok(validation_result) => {
                let validation_result: Tuple = validation_result.into();

                let validation_result_message: Str = validation_result.get(0).unwrap().into();
                let context_hash: OcamlHash = validation_result.get(1).unwrap().into();
                let block_header_proto_json: Str = validation_result.get(2).unwrap().into();
                let block_header_proto_metadata_json: Str = validation_result.get(3).unwrap().into();
                let operations_proto_metadata_json: Str = validation_result.get(4).unwrap().into();

                Ok(ApplyBlockResult {
                    validation_result_message: validation_result_message.as_str().to_string(),
                    context_hash: context_hash.convert_to(),
                    block_header_proto_json: block_header_proto_json.as_str().to_string(),
                    block_header_proto_metadata_json: block_header_proto_metadata_json.as_str().to_string(),
                    operations_proto_metadata_json: operations_proto_metadata_json.as_str().to_string(),
                })
            }
            Err(e) => {
                Err(ApplyBlockError::from(e))
            }
        }
    })
}

pub fn generate_identity(expected_pow: f64) -> Result<Result<Identity, TezosGenerateIdentityError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("generate_identity").expect("function 'generate_identity' is not registered");
        match ocaml_function.call_exn::<Value>(Value::f64(expected_pow)) {
            Ok(identity) => {
                let identity = Str::from(identity).as_str().to_string();

                Ok(serde_json::from_str::<Identity>(&identity)
                    .map_err(|err| TezosGenerateIdentityError::InvalidJsonError { message: err.to_string() })?
                )
            }
            Err(e) => {
                Err(TezosGenerateIdentityError::from(e))
            }
        }
    })
}

pub fn decode_context_data(protocol_hash: RustBytes, key: Vec<String>, data: RustBytes) -> Result<Result<Option<String>, ContextDataError>, OcamlError> {
    runtime::execute(move || {
        let mut key_list = List::new();
        key.iter()
            .rev()
            .for_each(|k| key_list.push_hd(Str::from(k.as_str()).into()));

        let ocaml_function = ocaml::named_value("decode_context_data").expect("function 'decode_context_data' is not registered");
        match ocaml_function.call3_exn::<OcamlHash, List, OcamlBytes>(protocol_hash.convert_to(), key_list, data.convert_to()) {
            Ok(decoded_data) => {
                let decoded_data: Str = decoded_data.into();
                if decoded_data.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(decoded_data.as_str().to_string()))
                }
            }
            Err(e) => {
                Err(ContextDataError::from(e))
            }
        }
    })
}

pub fn operations_to_ocaml(operations: &Vec<Option<Vec<RustBytes>>>) -> List {
    let mut operations_for_ocaml = List::new();

    operations.into_iter().rev()
        .for_each(|ops_option| {
            let ops_array = if let Some(ops) = ops_option {
                let mut ops_array = List::new();
                ops.into_iter().rev().for_each(|op| {
                    let op: OcamlBytes = op.convert_to();
                    ops_array.push_hd(Value::from(op));
                });
                ops_array
            } else {
                List::new()
            };
            operations_for_ocaml.push_hd(Value::from(ops_array));
        });

    operations_for_ocaml
}

pub fn protocol_overrides_to_ocaml(protocol_overrides: &ProtocolOverrides) -> Result<Tuple, ocaml::Error> {
    let mut forced_protocol_upgrades = List::new();
    protocol_overrides.forced_protocol_upgrades.iter().rev()
        .for_each(|(level, protocol_hash)| {
            let mut tuple: Tuple = Tuple::new(2);
            tuple.set(0, Value::int32(level.clone())).unwrap();
            tuple.set(1, Str::from(protocol_hash.as_str()).into()).unwrap();
            forced_protocol_upgrades.push_hd(Value::from(tuple));
        });

    let mut voted_protocol_overrides = List::new();
    protocol_overrides.voted_protocol_overrides.iter().rev()
        .for_each(|(protocol_hash1, protocol_hash2)| {
            let mut tuple: Tuple = Tuple::new(2);
            tuple.set(0, Str::from(protocol_hash1.as_str()).into()).unwrap();
            tuple.set(1, Str::from(protocol_hash2.as_str()).into()).unwrap();
            voted_protocol_overrides.push_hd(Value::from(tuple));
        });

    let mut protocol_overrides: Tuple = Tuple::new(2);
    protocol_overrides.set(0, Value::from(forced_protocol_upgrades))?;
    protocol_overrides.set(1, Value::from(voted_protocol_overrides))?;
    Ok(protocol_overrides)
}
