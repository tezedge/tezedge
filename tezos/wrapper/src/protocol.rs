// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{ChainId, ContextHash, ProtocolHash};
use tezos_api::ffi::*;

/// Provides trait that must be implemented by a protocol runner.
pub trait ProtocolApi {
    /// Apply block
    fn apply_block(request: ApplyBlockRequest) -> Result<ApplyBlockResponse, ApplyBlockError>;

    /// Begin construction new block
    fn begin_construction(request: BeginConstructionRequest) -> Result<PrevalidatorWrapper, BeginConstructionError>;

    /// Validate operation
    fn validate_operation(request: ValidateOperationRequest) -> Result<ValidateOperationResponse, ValidateOperationError>;

    /// Call protocol json rpc
    fn call_protocol_json_rpc(request: ProtocolJsonRpcRequest) -> Result<JsonRpcResponse, ProtocolRpcError>;

    /// Call helpers_preapply_operations shell service
    fn helpers_preapply_operations(request: ProtocolJsonRpcRequest) -> Result<JsonRpcResponse, ProtocolRpcError>;

    /// Call helpers_preapply_block shell service
    fn helpers_preapply_block(request: ProtocolJsonRpcRequest) -> Result<JsonRpcResponse, ProtocolRpcError>;

    /// Change tezos runtime configuration
    fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), TezosRuntimeConfigurationError>;

    /// Command tezos ocaml code to initialize protocol and context.
    fn init_protocol_context(
        storage_data_dir: String,
        genesis: GenesisChain,
        protocol_overrides: ProtocolOverrides,
        commit_genesis: bool,
        enable_testchain: bool,
        readonly: bool,
        patch_context: Option<PatchContext>) -> Result<InitProtocolContextResult, TezosStorageInitError>;

    /// Command gets genesis data from context
    fn genesis_result_data(
        genesis_context_hash: &ContextHash,
        chain_id: &ChainId,
        genesis_protocol_hash: &ProtocolHash,
        genesis_max_operations_ttl: u16,
    ) -> Result<CommitGenesisResult, GetDataError>;

    /// Command tezos ocaml code to compute the operations path
    fn compute_path(request: ComputePathRequest) -> Result<ComputePathResponse, ComputePathError>;
}
