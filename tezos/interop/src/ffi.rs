// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use ocaml::{List, Str, ToValue, Tuple, Value};
use ocaml::value::TRUE;
use serde_json;

use tezos_api::ffi::*;
use tezos_api::identity::Identity;

use crate::runtime;
use crate::runtime::OcamlError;

pub type OcamlBytes = Str;
pub type OcamlHash = OcamlBytes;

pub trait Interchange<T> {
    fn convert_to(&self) -> T;
    fn is_empty(&self) -> bool;
}

fn copy_to_ocaml(bytes: &[u8]) -> Str {
    let mut arr = Str::new(bytes.len());
    let data = arr.data_mut();
    for (n, i) in bytes.iter().enumerate() {
        data[n] = *i;
    }
    arr
}

impl Interchange<OcamlBytes> for RustBytes {
    fn convert_to(&self) -> OcamlBytes {
        copy_to_ocaml(self.as_slice())
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

pub fn init_protocol_context(
    storage_data_dir: String,
    genesis: GenesisChain,
    protocol_overrides: ProtocolOverrides,
    commit_genesis: bool,
    enable_testchain: bool)
    -> Result<Result<InitProtocolContextResult, TezosStorageInitError>, OcamlError> {
    runtime::execute(move || {
        // genesis configuration
        let mut genesis_tuple: Tuple = Tuple::new(3);
        genesis_tuple.set(0, Str::from(genesis.time.as_str()).into()).unwrap();
        genesis_tuple.set(1, Str::from(genesis.block.as_str()).into()).unwrap();
        genesis_tuple.set(2, Str::from(genesis.protocol.as_str()).into()).unwrap();

        // protocol overrides
        let protocol_overrides_tuple: Tuple = protocol_overrides_to_ocaml(protocol_overrides)?;

        let ocaml_function = ocaml::named_value("init_protocol_context").expect("function 'init_protocol_context' is not registered");
        match ocaml_function.call_n_exn(
            [
                Value::from(Str::from(storage_data_dir.as_str())),
                Value::from(genesis_tuple),
                Value::from(protocol_overrides_tuple),
                Value::bool(commit_genesis),
                Value::bool(enable_testchain)
            ]
        ) {
            Ok(result) => {
                let ocaml_result: Tuple = result.into();

                // 1. list known protocols
                let supported_protocol_hashes: List = ocaml_result.get(0).unwrap().into();
                let supported_protocol_hashes: Vec<RustBytes> = supported_protocol_hashes.to_vec()
                    .iter()
                    .map(|protocol_hash| {
                        let protocol_hash: OcamlBytes = protocol_hash.clone().into();
                        protocol_hash.convert_to()
                    })
                    .collect();

                // 2. context_hash option
                let genesis_commit_context_hash: OcamlHash = ocaml_result.get(1).unwrap().into();
                let genesis_commit_hash = if genesis_commit_context_hash.is_empty() {
                    None
                } else {
                    Some(genesis_commit_context_hash.convert_to())
                };

                Ok(InitProtocolContextResult {
                    supported_protocol_hashes,
                    genesis_commit_hash,
                })
            }
            Err(e) => {
                Err(TezosStorageInitError::from(e))
            }
        }
    })
}

pub fn genesis_result_data(context_hash: RustBytes, chain_id: RustBytes, protocol_hash: RustBytes, genesis_max_operations_ttl: u16) -> Result<Result<CommitGenesisResult, GetDataError>, OcamlError> {
    runtime::execute(move || {
        let context_hash: OcamlHash = context_hash.convert_to();
        let chain_id: OcamlHash = chain_id.convert_to();
        let protocol_hash: OcamlHash = protocol_hash.convert_to();

        let ocaml_function = ocaml::named_value("genesis_result_data").expect("function 'genesis_result_data' is not registered");
        match ocaml_function.call_n_exn(
            vec![
                context_hash.to_value(),
                chain_id.to_value(),
                protocol_hash.to_value(),
                Value::usize(genesis_max_operations_ttl as usize),
            ]
        ) {
            Ok(result) => {
                let ocaml_result: Tuple = result.into();

                // context_hash option
                let block_header_proto_json: Str = ocaml_result.get(0).unwrap().into();
                let block_header_proto_metadata_json: Str = ocaml_result.get(1).unwrap().into();
                let operations_proto_metadata_json: Str = ocaml_result.get(2).unwrap().into();
                Ok(
                    CommitGenesisResult {
                        block_header_proto_json: String::from(block_header_proto_json.as_str()),
                        block_header_proto_metadata_json: String::from(block_header_proto_metadata_json.as_str()),
                        operations_proto_metadata_json: String::from(operations_proto_metadata_json.as_str()),
                    }
                )
            }
            Err(e) => {
                Err(GetDataError::from(e))
            }
        }
    })
}

/// Applies block to context
/// - apply_block_request see [tezos_api::ffi:ApplyBlockRequest]
pub fn apply_block(apply_block_request: RustBytes)
    -> Result<Result<ApplyBlockResult, ApplyBlockError>, OcamlError> {
    runtime::execute(move || {
        let ocaml_function = ocaml::named_value("apply_block").expect("function 'apply_block' is not registered");

        // call ffi
        match ocaml_function.call_exn::<OcamlBytes>(apply_block_request.convert_to()) {
            Ok(validation_result) => {
                let validation_result: Tuple = validation_result.into();

                let validation_result_message: Str = validation_result.get(0).unwrap().into();
                let context_hash: OcamlHash = validation_result.get(1).unwrap().into();
                let block_header_proto_json: Str = validation_result.get(2).unwrap().into();
                let block_header_proto_metadata_json: Str = validation_result.get(3).unwrap().into();
                let operations_proto_metadata_json: Str = validation_result.get(4).unwrap().into();
                let max_operations_ttl: u16 = validation_result.get(5).unwrap().usize_val() as u16;
                let last_allowed_fork_level: i32 = validation_result.get(6).unwrap().int32_val();
                let forking_testchain = validation_result.get(7).unwrap().usize_val() == TRUE.usize_val();
                let forking_testchain_data: Option<ForkingTestchainData> = if forking_testchain {
                    let test_chain: Tuple = validation_result.get(8).unwrap().into();
                    let genesis: OcamlHash = test_chain.get(0).unwrap().into();
                    let chain_id: OcamlHash = test_chain.get(1).unwrap().into();
                    Some(ForkingTestchainData {
                        genesis: genesis.convert_to(),
                        chain_id: chain_id.convert_to(),
                    })
                } else {
                    None
                };

                Ok(ApplyBlockResult {
                    validation_result_message: validation_result_message.as_str().to_string(),
                    context_hash: context_hash.convert_to(),
                    block_header_proto_json: block_header_proto_json.as_str().to_string(),
                    block_header_proto_metadata_json: block_header_proto_metadata_json.as_str().to_string(),
                    operations_proto_metadata_json: operations_proto_metadata_json.as_str().to_string(),
                    max_operations_ttl,
                    last_allowed_fork_level,
                    forking_testchain,
                    forking_testchain_data,
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

pub fn protocol_overrides_to_ocaml(protocol_overrides: ProtocolOverrides) -> Result<Tuple, ocaml::Error> {
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
