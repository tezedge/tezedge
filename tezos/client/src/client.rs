// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{ChainId, ContextHash, ProtocolHash};
use tezos_api::ffi::{
    ApplyBlockError, ApplyBlockRequest, ApplyBlockResponse, BeginApplicationError,
    BeginApplicationRequest, BeginApplicationResponse, BeginConstructionError,
    BeginConstructionRequest, CommitGenesisResult, ComputePathError, ComputePathRequest,
    ComputePathResponse, ContextDataError, GenesisChain, GetDataError, HelpersPreapplyBlockRequest,
    HelpersPreapplyError, HelpersPreapplyResponse, InitProtocolContextResult, PatchContext,
    PrevalidatorWrapper, ProtocolDataError, ProtocolOverrides, ProtocolRpcError,
    ProtocolRpcRequest, ProtocolRpcResponse, TezosRuntimeConfiguration,
    TezosRuntimeConfigurationError, TezosStorageInitError, ValidateOperationError,
    ValidateOperationRequest, ValidateOperationResponse,
};
use tezos_interop::ffi;

/// Override runtime configuration for OCaml runtime
pub fn change_runtime_configuration(
    settings: TezosRuntimeConfiguration,
) -> Result<(), TezosRuntimeConfigurationError> {
    match ffi::change_runtime_configuration(settings) {
        Ok(result) => Ok(result?),
        Err(e) => Err(TezosRuntimeConfigurationError::ChangeConfigurationError {
            message: format!("FFI 'change_runtime_configuration' failed! Reason: {:?}", e),
        }),
    }
}

/// Initializes context for Tezos ocaml protocol
pub fn init_protocol_context(
    storage_data_dir: String,
    genesis: GenesisChain,
    protocol_overrides: ProtocolOverrides,
    commit_genesis: bool,
    enable_testchain: bool,
    readonly: bool,
    patch_context: Option<PatchContext>,
) -> Result<InitProtocolContextResult, TezosStorageInitError> {
    match ffi::init_protocol_context(storage_data_dir, genesis, protocol_overrides, commit_genesis, enable_testchain, readonly, patch_context) {
        Ok(result) => Ok(result?),
        Err(e) => {
            Err(TezosStorageInitError::InitializeError {
                message: format!("FFI 'init_protocol_context' failed! Initialization of Tezos context failed, this storage is required, we can do nothing without that! Reason: {:?}", e)
            })
        }
    }
}

/// Gets data for genesis
pub fn genesis_result_data(
    context_hash: &ContextHash,
    chain_id: &ChainId,
    protocol_hash: &ProtocolHash,
    genesis_max_operations_ttl: u16,
) -> Result<CommitGenesisResult, GetDataError> {
    match ffi::genesis_result_data(
        context_hash.clone(),
        chain_id.clone(),
        protocol_hash.clone(),
        genesis_max_operations_ttl,
    ) {
        Ok(result) => Ok(result?),
        Err(e) => Err(GetDataError::ReadError {
            message: format!("FFI 'genesis_result_data' failed! Reason: {:?}", e),
        }),
    }
}

/// Applies new block to Tezos ocaml storage, means:
/// - block and operations are decoded by the protocol
/// - block and operations data are correctly stored in Tezos chain/storage
/// - new current head is evaluated
/// - returns validation_result.message
pub fn apply_block(request: ApplyBlockRequest) -> Result<ApplyBlockResponse, ApplyBlockError> {
    // check operations count by validation_pass
    if (request.block_header.validation_pass() as usize) != request.operations.len() {
        return Err(ApplyBlockError::IncompleteOperations {
            expected: request.block_header.validation_pass() as usize,
            actual: request.operations.len(),
        });
    }

    match ffi::apply_block(request) {
        Ok(result) => result.map_err(ApplyBlockError::from),
        Err(e) => Err(ApplyBlockError::FailedToApplyBlock {
            message: format!("Unknown OcamlError: {:?}", e),
        }),
    }
}

/// Begin application
pub fn begin_application(
    request: BeginApplicationRequest,
) -> Result<BeginApplicationResponse, BeginApplicationError> {
    match ffi::begin_application(request) {
        Ok(result) => result.map_err(BeginApplicationError::from),
        Err(e) => Err(BeginApplicationError::FailedToBeginApplication {
            message: format!("Unknown OcamlError: {:?}", e),
        }),
    }
}

/// Begin construction
pub fn begin_construction(
    request: BeginConstructionRequest,
) -> Result<PrevalidatorWrapper, BeginConstructionError> {
    match ffi::begin_construction(request) {
        Ok(result) => result.map_err(BeginConstructionError::from),
        Err(e) => Err(BeginConstructionError::FailedToBeginConstruction {
            message: format!("Unknown OcamlError: {:?}", e),
        }),
    }
}

/// Validate operation
pub fn validate_operation(
    request: ValidateOperationRequest,
) -> Result<ValidateOperationResponse, ValidateOperationError> {
    match ffi::validate_operation(request) {
        Ok(result) => result.map_err(ValidateOperationError::from),
        Err(e) => Err(ValidateOperationError::FailedToValidateOperation {
            message: format!("Unknown OcamlError: {:?}", e),
        }),
    }
}

/// Call protocol rpc - general service
pub fn call_protocol_rpc(
    request: ProtocolRpcRequest,
) -> Result<ProtocolRpcResponse, ProtocolRpcError> {
    match ffi::call_protocol_rpc(request) {
        Ok(result) => result,
        Err(e) => Err(ProtocolRpcError::FailedToCallProtocolRpc(format!(
            "Unknown OcamlError: {:?}",
            e
        ))),
    }
}

/// Call compute path
/// TODO: TE-207 Implement in Rust
pub fn compute_path(request: ComputePathRequest) -> Result<ComputePathResponse, ComputePathError> {
    match ffi::compute_path(request) {
        Ok(result) => Ok(result?),
        Err(e) => Err(ComputePathError::PathError {
            message: format!("Path computation failed! Reason: {:?}", e),
        }),
    }
}

/// Call helpers_preapply_operations shell service
pub fn helpers_preapply_operations(
    request: ProtocolRpcRequest,
) -> Result<HelpersPreapplyResponse, HelpersPreapplyError> {
    match ffi::helpers_preapply_operations(request) {
        Ok(result) => result.map_err(HelpersPreapplyError::from),
        Err(e) => Err(HelpersPreapplyError::FailedToCallProtocolRpc {
            message: format!("Unknown OcamlError: {:?}", e),
        }),
    }
}

/// Call helpers_preapply_block shell service
pub fn helpers_preapply_block(
    request: HelpersPreapplyBlockRequest,
) -> Result<HelpersPreapplyResponse, HelpersPreapplyError> {
    match ffi::helpers_preapply_block(request) {
        Ok(result) => result.map_err(HelpersPreapplyError::from),
        Err(e) => Err(HelpersPreapplyError::FailedToCallProtocolRpc {
            message: format!("Unknown OcamlError: {:?}", e),
        }),
    }
}

/// Decode protocoled context data
pub fn decode_context_data(
    protocol_hash: ProtocolHash,
    key: Vec<String>,
    data: Vec<u8>,
) -> Result<Option<String>, ContextDataError> {
    match ffi::decode_context_data(protocol_hash, key, data) {
        Ok(result) => Ok(result?),
        Err(e) => Err(ContextDataError::DecodeError {
            message: format!("FFI 'decode_context_data' failed! Reason: {:?}", e),
        }),
    }
}

/// Decode protocoled context data
pub fn assert_encoding_for_protocol_data(
    protocol_hash: ProtocolHash,
    protocol_data: Vec<u8>,
) -> Result<(), ProtocolDataError> {
    match ffi::assert_encoding_for_protocol_data(protocol_hash, protocol_data) {
        Ok(result) => Ok(result?),
        Err(e) => Err(ProtocolDataError::DecodeError {
            message: format!(
                "FFI 'assert_encoding_for_protocol_data' failed! Reason: {:?}",
                e
            ),
        }),
    }
}

pub fn shutdown_runtime() {
    ffi::shutdown();
}
