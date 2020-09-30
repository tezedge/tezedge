// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Once;

use ocaml_interop::{
    ocaml_alloc, ocaml_call, ocaml_frame, to_ocaml, FromOCaml, IntoRust, OCaml, OCamlBytes,
    OCamlFn1, OCamlInt32, OCamlList, ToOCaml,
};

use tezos_api::ffi::*;
use tezos_api::{ocaml_conv::FfiPath};

use crate::runtime;
use crate::runtime::OcamlError;

mod tezos_ffi {
    use tezos_api::{
        ffi::{
            ApplyBlockRequest, ApplyBlockResponse, BeginConstructionRequest, JsonRpcResponse,
            PrevalidatorWrapper, ProtocolJsonRpcRequest, ValidateOperationRequest,
            ValidateOperationResponse,
        },
        ocaml_conv::OCamlOperationHash,
    };
    use tezos_messages::p2p::encoding::operations_for_blocks::Path;
    use ocaml_interop::{ocaml, OCamlBytes, OCamlInt, OCamlInt32, OCamlList};

    ocaml! {
        pub fn apply_block(apply_block_request: ApplyBlockRequest) -> ApplyBlockResponse;
        pub fn begin_construction(begin_construction_request: BeginConstructionRequest) -> PrevalidatorWrapper;
        pub fn validate_operation(validate_operation_request: ValidateOperationRequest) -> ValidateOperationResponse;
        pub fn call_protocol_json_rpc(request: ProtocolJsonRpcRequest) -> JsonRpcResponse;
        pub fn helpers_preapply_operations(request: ProtocolJsonRpcRequest) -> JsonRpcResponse;
        pub fn helpers_preapply_block(request: ProtocolJsonRpcRequest) -> JsonRpcResponse;
        pub fn change_runtime_configuration(
            log_enabled: bool,
            no_of_ffi_calls_treshold_for_gc: OCamlInt,
            debug_mode: bool
        );
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
            genesis_max_operations_ttl: OCamlInt
        ) -> (OCamlBytes, OCamlBytes, OCamlBytes);
        pub fn decode_context_data(
            protocol_hash: OCamlBytes,
            key: OCamlList<OCamlBytes>,
            data: OCamlBytes
        ) -> Option<OCamlBytes>;
        pub fn compute_path(request: OCamlList<OCamlList<OCamlOperationHash>>) -> OCamlList<Path>;
    }
}

/// Initializes the ocaml runtime and the tezos-ffi callback mechanism.
pub fn setup() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        ocaml_interop::OCamlRuntime::init_persistent();
        tezos_interop_callback::initialize_callbacks();
    });
}

/// Tries to shutdown ocaml runtime gracefully - give chance to close resources, trigger GC finalization...
///
/// https://caml.inria.fr/pub/docs/manual-ocaml/intfc.html#sec467
pub fn shutdown() {
    ocaml_interop::OCamlRuntime::shutdown_persistent()
}

pub fn change_runtime_configuration(
    settings: TezosRuntimeConfiguration,
) -> Result<Result<(), TezosRuntimeConfigurationError>, OcamlError> {
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let result = ocaml_call!(tezos_ffi::change_runtime_configuration(
                gc,
                OCaml::of_bool(settings.log_enabled),
                OCaml::of_i32(settings.no_of_ffi_calls_treshold_for_gc),
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
            let ref context_hash = to_ocaml!(gc, context_hash).keep(gc);
            let ref chain_id = to_ocaml!(gc, chain_id).keep(gc);
            let protocol_hash = ocaml_alloc!(protocol_hash.to_ocaml(gc));
            let genesis_max_operations_ttl = OCaml::of_i32(genesis_max_operations_ttl as i32);

            let result = ocaml_call!(tezos_ffi::genesis_result_data(
                gc,
                gc.get(context_hash),
                gc.get(chain_id),
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

type CallRequestFn<REQUEST, RESPONSE> = OCamlFn1<REQUEST, RESPONSE>;

/// Calls ffi function like request/response
pub fn call<REQUEST, RESPONSE>(
    ocaml_function: CallRequestFn<REQUEST, RESPONSE>,
    request: REQUEST,
) -> Result<Result<RESPONSE, CallError>, OcamlError>
where
    REQUEST: ToOCaml<REQUEST> + Send + 'static,
    RESPONSE: FromOCaml<RESPONSE> + Send + 'static,
{
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let ocaml_request = to_ocaml!(gc, request);
            let result = ocaml_call!(ocaml_function(gc, ocaml_request));
            match result {
                Ok(response) => Ok(response.into_rust()),
                Err(e) => Err(CallError::from(e)),
            }
        })
    })
}

/// Applies block to context
pub fn apply_block(
    request: ApplyBlockRequest,
) -> Result<Result<ApplyBlockResponse, CallError>, OcamlError> {
    call(tezos_ffi::apply_block, request)
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
    runtime::execute(move || {
        ocaml_frame!(gc, {
            let ocaml_request = to_ocaml!(gc, request.operations);
            let result = ocaml_call!(tezos_ffi::compute_path(gc, ocaml_request));
            match result {
                Ok(response) => {
                    let operations_hashes_path: Vec<FfiPath> = response.into_rust();
                    let operations_hashes_path = operations_hashes_path
                        .into_iter()
                        .map(|path| path.0)
                        .collect();
                    Ok(ComputePathResponse {
                        operations_hashes_path,
                    })
                }
                Err(e) => Err(CallError::from(e)),
            }
        })
    })
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
