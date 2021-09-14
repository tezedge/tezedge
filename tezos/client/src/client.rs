// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{ChainId, ContextHash, ProtocolHash};
use tezos_api::ffi::{
    ApplyBlockError, ApplyBlockRequest, ApplyBlockResponse, BeginApplicationError,
    BeginApplicationRequest, BeginApplicationResponse, BeginConstructionError,
    BeginConstructionRequest, CommitGenesisResult, ComputePathError, ComputePathRequest,
    ComputePathResponse, ContextDataError, FfiJsonEncoderError, GetDataError,
    HelpersPreapplyBlockRequest, HelpersPreapplyError, HelpersPreapplyResponse,
    InitProtocolContextResult, PrevalidatorWrapper, ProtocolDataError, ProtocolRpcError,
    ProtocolRpcRequest, ProtocolRpcResponse, RustBytes, TezosRuntimeConfiguration,
    TezosRuntimeConfigurationError, TezosStorageInitError, ValidateOperationError,
    ValidateOperationRequest, ValidateOperationResponse,
};
use tezos_context_api::TezosContextConfiguration;
use tezos_interop::ffi;
use tezos_messages::p2p::encoding::operation::Operation;

/// Override runtime configuration for OCaml runtime
pub fn change_runtime_configuration(
    settings: TezosRuntimeConfiguration,
) -> Result<(), TezosRuntimeConfigurationError> {
    ffi::change_runtime_configuration(settings).map_err(|e| {
        TezosRuntimeConfigurationError::ChangeConfigurationError {
            message: format!("FFI 'change_runtime_configuration' failed, reason: {:?}", e),
        }
    })
}

/// Initializes context for Tezos ocaml protocol
pub fn init_protocol_context(
    context_config: TezosContextConfiguration,
) -> Result<InitProtocolContextResult, TezosStorageInitError> {
    ffi::init_protocol_context(
        context_config
    ).map_err(|e| {
        TezosStorageInitError::InitializeError {
            message: format!("FFI 'init_protocol_context' failed! Initialization of Tezos context failed, this storage is required, we can do nothing without that, reason: {:?}", e)
        }
    })
}

/// Gets data for genesis
pub fn genesis_result_data(
    context_hash: &ContextHash,
    chain_id: &ChainId,
    protocol_hash: &ProtocolHash,
    genesis_max_operations_ttl: u16,
) -> Result<CommitGenesisResult, GetDataError> {
    ffi::genesis_result_data(
        context_hash.as_ref().clone(),
        chain_id.as_ref().clone(),
        protocol_hash.as_ref().clone(),
        genesis_max_operations_ttl,
    )
    .map_err(|e| GetDataError::ReadError {
        message: format!("FFI 'genesis_result_data' failed, reason: {:?}", e),
    })
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

    ffi::apply_block(request).map_err(ApplyBlockError::from)
}

/// Begin application
pub fn begin_application(
    request: BeginApplicationRequest,
) -> Result<BeginApplicationResponse, BeginApplicationError> {
    ffi::begin_application(request).map_err(BeginApplicationError::from)
}

/// Begin construction
pub fn begin_construction(
    request: BeginConstructionRequest,
) -> Result<PrevalidatorWrapper, BeginConstructionError> {
    ffi::begin_construction(request).map_err(BeginConstructionError::from)
}

/// Validate operation
pub fn validate_operation(
    request: ValidateOperationRequest,
) -> Result<ValidateOperationResponse, ValidateOperationError> {
    ffi::validate_operation(request).map_err(ValidateOperationError::from)
}

/// Call protocol rpc - general service
pub fn call_protocol_rpc(
    request: ProtocolRpcRequest,
) -> Result<ProtocolRpcResponse, ProtocolRpcError> {
    ffi::call_protocol_rpc(request)
}

/// Call compute path
/// TODO: TE-207 Implement in Rust
pub fn compute_path(request: ComputePathRequest) -> Result<ComputePathResponse, ComputePathError> {
    ffi::compute_path(request).map_err(|e| ComputePathError::PathError {
        message: format!("Path computation failed, reason: {:?}", e),
    })
}

/// Call helpers_preapply_operations shell service
pub fn helpers_preapply_operations(
    request: ProtocolRpcRequest,
) -> Result<HelpersPreapplyResponse, HelpersPreapplyError> {
    ffi::helpers_preapply_operations(request).map_err(HelpersPreapplyError::from)
}

/// Call helpers_preapply_block shell service
pub fn helpers_preapply_block(
    request: HelpersPreapplyBlockRequest,
) -> Result<HelpersPreapplyResponse, HelpersPreapplyError> {
    ffi::helpers_preapply_block(request).map_err(HelpersPreapplyError::from)
}

/// Decode protocoled context data
pub fn decode_context_data(
    protocol_hash: ProtocolHash,
    key: Vec<String>,
    data: Vec<u8>,
) -> Result<Option<String>, ContextDataError> {
    ffi::decode_context_data(protocol_hash.into(), key, data).map_err(|e| {
        ContextDataError::DecodeError {
            message: format!("FFI 'decode_context_data' failed, reason: {:?}", e),
        }
    })
}

/// Decode protocoled context data
pub fn assert_encoding_for_protocol_data(
    protocol_hash: ProtocolHash,
    protocol_data: Vec<u8>,
) -> Result<(), ProtocolDataError> {
    ffi::assert_encoding_for_protocol_data(protocol_hash.into(), protocol_data).map_err(|e| {
        ProtocolDataError::DecodeError {
            message: format!(
                "FFI 'assert_encoding_for_protocol_data' failed, reason: {:?}",
                e
            ),
        }
    })
}

/// Encode apply_block result metadata as JSON
pub fn apply_block_result_metadata(
    context_hash: ContextHash,
    metadata_bytes: RustBytes,
    max_operations_ttl: i32,
    protocol_hash: ProtocolHash,
    next_protocol_hash: ProtocolHash,
) -> Result<String, FfiJsonEncoderError> {
    ffi::apply_block_result_metadata(
        context_hash,
        metadata_bytes,
        max_operations_ttl,
        protocol_hash,
        next_protocol_hash,
    )
}

/// Encode apply_block result operations metadata as JSON
pub fn apply_block_operations_metadata(
    chain_id: ChainId,
    operations: Vec<Vec<Operation>>,
    operations_metadata_bytes: Vec<Vec<RustBytes>>,
    protocol_hash: ProtocolHash,
    next_protocol_hash: ProtocolHash,
) -> Result<String, FfiJsonEncoderError> {
    ffi::apply_block_operations_metadata(
        chain_id,
        operations,
        operations_metadata_bytes,
        protocol_hash,
        next_protocol_hash,
    )
}

/// Shutdown the OCaml runtime
pub fn shutdown_runtime() {
    ffi::shutdown()
}
