// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{ChainId, ContextHash, ProtocolHash};
use tezos_api::ffi::*;
use tezos_context_api::TezosContextConfiguration;
use tezos_messages::p2p::encoding::operation::Operation;

/// Provides trait that must be implemented by a protocol runner.
pub trait ProtocolApi {
    /// Apply block
    fn apply_block(request: ApplyBlockRequest) -> Result<ApplyBlockResponse, ApplyBlockError>;

    /// Begin application new block
    fn begin_application(
        request: BeginApplicationRequest,
    ) -> Result<BeginApplicationResponse, BeginApplicationError>;

    /// Begin construction new block
    fn begin_construction(
        request: BeginConstructionRequest,
    ) -> Result<PrevalidatorWrapper, BeginConstructionError>;

    /// Validate operation
    fn validate_operation(
        request: ValidateOperationRequest,
    ) -> Result<ValidateOperationResponse, ValidateOperationError>;

    /// Call protocol rpc
    fn call_protocol_rpc(
        request: ProtocolRpcRequest,
    ) -> Result<ProtocolRpcResponse, ProtocolRpcError>;

    /// Call helpers_preapply_operations shell service
    fn helpers_preapply_operations(
        request: ProtocolRpcRequest,
    ) -> Result<HelpersPreapplyResponse, HelpersPreapplyError>;

    /// Call helpers_preapply_block shell service
    fn helpers_preapply_block(
        request: HelpersPreapplyBlockRequest,
    ) -> Result<HelpersPreapplyResponse, HelpersPreapplyError>;

    /// Change tezos runtime configuration
    fn change_runtime_configuration(
        settings: TezosRuntimeConfiguration,
    ) -> Result<(), TezosRuntimeConfigurationError>;

    /// Command tezos ocaml code to initialize protocol and context.
    fn init_protocol_context(
        context_config: TezosContextConfiguration,
    ) -> Result<InitProtocolContextResult, TezosStorageInitError>;

    /// Command gets genesis data from context
    fn genesis_result_data(
        genesis_context_hash: &ContextHash,
        chain_id: &ChainId,
        genesis_protocol_hash: &ProtocolHash,
        genesis_max_operations_ttl: u16,
    ) -> Result<CommitGenesisResult, GetDataError>;

    /// Command tezos ocaml code to compute the operations path
    fn compute_path(request: ComputePathRequest) -> Result<ComputePathResponse, ComputePathError>;

    /// Verify if block_header's protocol_data can be encoded by protocol_hash
    fn assert_encoding_for_protocol_data(
        protocol_hash: ProtocolHash,
        protocol_data: Vec<u8>,
    ) -> Result<(), ProtocolDataError>;

    /// Encode apply_block result metadata as JSON
    fn apply_block_result_metadata(
        context_hash: ContextHash,
        metadata_bytes: RustBytes,
        max_operations_ttl: i32,
        protocol_hash: ProtocolHash,
        next_protocol_hash: ProtocolHash,
    ) -> Result<String, FfiJsonEncoderError>;

    /// Encode apply_block result operations metadata as JSON
    fn apply_block_operations_metadata(
        chain_id: ChainId,
        operations: Vec<Vec<Operation>>,
        operations_metadata_bytes: Vec<Vec<RustBytes>>,
        protocol_hash: ProtocolHash,
        next_protocol_hash: ProtocolHash,
    ) -> Result<String, FfiJsonEncoderError>;
}
