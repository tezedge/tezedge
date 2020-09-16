// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Once;

use znfe::{
    IntoRust, OCaml, ocaml_alloc, ocaml_call, ocaml_frame, OCamlBytes, OCamlFn1, OCamlInt32,
    OCamlList, ToOCaml,
};

use tezos_api::ffi::*;

use crate::runtime;
use crate::runtime::OcamlError;

mod tezos_ffi {
    use znfe::{Intnat, ocaml, OCamlBytes, OCamlInt32, OCamlList};

    ocaml! {
        pub fn apply_block(apply_block_request: OCamlBytes) -> OCamlBytes;
        pub fn begin_construction(begin_construction_request: OCamlBytes) -> OCamlBytes;
        pub fn validate_operation(validate_operation_request: OCamlBytes) -> OCamlBytes;
        pub fn call_protocol_json_rpc(request: OCamlBytes) -> OCamlBytes;
        pub fn helpers_preapply_operations(request: OCamlBytes) -> OCamlBytes;
        pub fn helpers_preapply_block(request: OCamlBytes) -> OCamlBytes;
        pub fn change_runtime_configuration(
            log_enabled: bool,
            no_of_ffi_calls_treshold_for_gc: Intnat,
            debug_mode: bool
        ) -> ();
        pub fn init_protocol_context(
            data_dir: String,
            genesis: (OCamlBytes, OCamlBytes, OCamlBytes),
            protocol_override: (OCamlList<(OCamlInt32, OCamlBytes)>,
                                OCamlList<(OCamlBytes, OCamlBytes)>),
            configuration: (bool, bool, bool),
            sandbox_json_patch_context: Option<(OCamlBytes, OCamlBytes)>
        ) -> (OCamlList<OCamlBytes>, Option<OCamlBytes>);
        pub fn genesis_result_data(
            context_hash: OCamlBytes,
            chain_id: OCamlBytes,
            protocol_hash: OCamlBytes,
            genesis_max_operations_ttl: Intnat
        ) -> (OCamlBytes, OCamlBytes, OCamlBytes);
        pub fn decode_context_data(
            protocol_hash: OCamlBytes,
            key: OCamlList<OCamlBytes>,
            data: OCamlBytes
        ) -> Option<OCamlBytes>;
        pub fn compute_path(request: OCamlBytes) -> OCamlBytes;
    }
}

/// Initializes the ocaml runtime and the tezos-ffi callback mechanism.
pub fn setup() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        znfe::OCamlRuntime::init_persistent();
        tezos_interop_callback::initialize_callbacks();
    });
}

/// Tries to shutdown ocaml runtime gracefully - give chance to close resources, trigger GC finalization...
///
/// https://caml.inria.fr/pub/docs/manual-ocaml/intfc.html#sec467
pub fn shutdown() {
    znfe::OCamlRuntime::shutdown_persistent()
}

type CallRequestFn = OCamlFn1<OCamlBytes, OCamlBytes>;

/// Calls ffi function like request/response
pub fn call<REQUEST, RESPONSE>(
    ocaml_function: CallRequestFn,
    request: REQUEST,
) -> Result<Result<RESPONSE, CallError>, OcamlError>
    where
        REQUEST: FfiMessage + 'static,
        RESPONSE: FfiMessage + 'static,
{
    runtime::execute(move || {
        // write to bytes
        let request = match request.as_rust_bytes() {
            Ok(data) => data,
            Err(e) => {
                return Err(CallError::InvalidRequestData {
                    message: format!("{:?}", e),
                });
            }
        };

        // call ffi
        ocaml_frame!(gc, {
            let request = ocaml_alloc!(request.to_ocaml(gc));
            let result = ocaml_call!(ocaml_function(gc, request));
            match result {
                Ok(response) => {
                    let response = response.into_rust();

                    let response = RESPONSE::from_rust_bytes(response).map_err(|error| {
                        CallError::InvalidResponseData {
                            message: format!("{}", error),
                        }
                    })?;
                    Ok(response)
                }
                Err(e) => Err(CallError::from(e)),
            }
        })
    })
}

pub fn change_runtime_configuration(
    settings: TezosRuntimeConfiguration,
) -> Result<Result<(), TezosRuntimeConfigurationError>, OcamlError> {
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let result = ocaml_call!(tezos_ffi::change_runtime_configuration(
                gc,
                OCaml::of_bool(settings.log_enabled),
                OCaml::of_int(settings.no_of_ffi_calls_treshold_for_gc as i64),
                OCaml::of_bool(settings.debug_mode)
            ));
            match result {
                Ok(_) => Ok(()),
                Err(e) => Err(TezosRuntimeConfigurationError::from(e)),
            }
        })
    })
}

pub fn init_protocol_context(
    storage_data_dir: String,
    genesis: GenesisChain,
    protocol_overrides: ProtocolOverrides,
    commit_genesis: bool,
    enable_testchain: bool,
    readonly: bool,
    patch_context: Option<PatchContext>,
) -> Result<Result<InitProtocolContextResult, TezosStorageInitError>, OcamlError> {
    runtime::execute(move || {
        ocaml_frame!(gc, {
            // genesis configuration
            let genesis_tuple = (genesis.time, genesis.block, genesis.protocol);
            let genesis_tuple = ocaml_alloc!(genesis_tuple.to_ocaml(gc));
            let ref genesis_tuple_ref = gc.keep(genesis_tuple);

            // protocol overrides
            let protocol_overrides_tuple = (
                protocol_overrides.forced_protocol_upgrades,
                protocol_overrides.voted_protocol_overrides,
            );
            let protocol_overrides_tuple: OCaml<(
                OCamlList<(OCamlInt32, OCamlBytes)>,
                OCamlList<(OCamlBytes, OCamlBytes)>,
            )> = ocaml_alloc!(protocol_overrides_tuple.to_ocaml(gc));
            let ref protocol_overrides_tuple_ref = gc.keep(protocol_overrides_tuple);

            // configuration
            let configuration = (commit_genesis, enable_testchain, readonly);
            let configuration = ocaml_alloc!(configuration.to_ocaml(gc));
            let ref configuration_ref = gc.keep(configuration);

            // patch context
            let patch_context_tuple = patch_context.map(|pc| (pc.key, pc.json));
            let patch_context_tuple = ocaml_alloc!(patch_context_tuple.to_ocaml(gc));
            let ref patch_context_tuple_ref = gc.keep(patch_context_tuple);

            let storage_data_dir = ocaml_alloc!(storage_data_dir.to_ocaml(gc));
            let result = ocaml_call!(tezos_ffi::init_protocol_context(
                gc,
                storage_data_dir,
                gc.get(genesis_tuple_ref),
                gc.get(protocol_overrides_tuple_ref),
                gc.get(configuration_ref),
                gc.get(patch_context_tuple_ref)
            ));

            match result {
                Ok(result) => {
                    let (supported_protocol_hashes, genesis_commit_hash): (
                        Vec<RustBytes>,
                        Option<RustBytes>,
                    ) = result.into_rust();

                    Ok(InitProtocolContextResult {
                        supported_protocol_hashes,
                        genesis_commit_hash,
                    })
                }
                Err(e) => Err(TezosStorageInitError::from(e)),
            }
        })
    })
}

pub fn genesis_result_data(
    context_hash: RustBytes,
    chain_id: RustBytes,
    protocol_hash: RustBytes,
    genesis_max_operations_ttl: u16,
) -> Result<Result<CommitGenesisResult, GetDataError>, OcamlError> {
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let context_hash = ocaml_alloc!(context_hash.to_ocaml(gc));
            let ref context_hash_ref = gc.keep(context_hash);
            let chain_id = ocaml_alloc!(chain_id.to_ocaml(gc));
            let ref chain_id_ref = gc.keep(chain_id);
            let protocol_hash = ocaml_alloc!(protocol_hash.to_ocaml(gc));
            let genesis_max_operations_ttl = OCaml::of_int(genesis_max_operations_ttl as i64);

            let result = ocaml_call!(tezos_ffi::genesis_result_data(
                gc,
                gc.get(context_hash_ref),
                gc.get(chain_id_ref),
                protocol_hash,
                genesis_max_operations_ttl
            ));
            match result {
                Ok(result) => {
                    let (
                        block_header_proto_json,
                        block_header_proto_metadata_json,
                        operations_proto_metadata_json,
                    ) = result.into_rust();
                    Ok(CommitGenesisResult {
                        block_header_proto_json,
                        block_header_proto_metadata_json,
                        operations_proto_metadata_json,
                    })
                }
                Err(e) => Err(GetDataError::from(e)),
            }
        })
    })
}

/// Applies block to context
pub fn apply_block(
    apply_block_request: ApplyBlockRequest,
) -> Result<Result<ApplyBlockResponse, CallError>, OcamlError> {
    call(tezos_ffi::apply_block, apply_block_request)
}

/// Begin construction initializes prevalidator and context for new operations based on current head
pub fn begin_construction(
    request: BeginConstructionRequest,
) -> Result<Result<PrevalidatorWrapper, CallError>, OcamlError> {
    call(tezos_ffi::begin_construction, request)
}

/// Validate operation - used with prevalidator for validation of operation
pub fn validate_operation(
    request: ValidateOperationRequest,
) -> Result<Result<ValidateOperationResponse, CallError>, OcamlError> {
    call(tezos_ffi::validate_operation, request)
}

/// Call protocol json rpc - general service
pub fn call_protocol_json_rpc(
    request: ProtocolJsonRpcRequest,
) -> Result<Result<JsonRpcResponse, CallError>, OcamlError> {
    call(tezos_ffi::call_protocol_json_rpc, request)
}

/// Call helpers_preapply_operations shell service
pub fn helpers_preapply_operations(
    request: ProtocolJsonRpcRequest,
) -> Result<Result<JsonRpcResponse, CallError>, OcamlError> {
    call(tezos_ffi::helpers_preapply_operations, request)
}

/// Call helpers_preapply_block shell service
pub fn helpers_preapply_block(
    request: ProtocolJsonRpcRequest,
) -> Result<Result<JsonRpcResponse, CallError>, OcamlError> {
    call(tezos_ffi::helpers_preapply_block, request)
}

/// Call compute path
pub fn compute_path(
    request: ComputePathRequest,
) -> Result<Result<ComputePathResponse, CallError>, OcamlError> {
    call(tezos_ffi::compute_path, request)
}

pub fn decode_context_data(
    protocol_hash: RustBytes,
    key: Vec<String>,
    data: RustBytes,
) -> Result<Result<Option<String>, ContextDataError>, OcamlError> {
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let protocol_hash = ocaml_alloc!(protocol_hash.to_ocaml(gc));
            let ref protocol_hash_ref = gc.keep(protocol_hash);
            let key_list: OCaml<OCamlList<OCamlBytes>> = ocaml_alloc!(key.to_ocaml(gc));
            let ref key_list_ref = gc.keep(key_list);
            let data = ocaml_alloc!(data.to_ocaml(gc));

            let result = ocaml_call!(tezos_ffi::decode_context_data(
                gc,
                gc.get(protocol_hash_ref),
                gc.get(key_list_ref),
                data
            ));

            match result {
                Ok(decoded_data) => {
                    let decoded_data = decoded_data.into_rust();
                    Ok(decoded_data)
                }
                Err(e) => Err(ContextDataError::from(e)),
            }
        })
    })
}
