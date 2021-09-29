// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use crypto::hash::{ChainId, ContextHash, ProtocolHash};
use serde::{Deserialize, Serialize};
use strum_macros::IntoStaticStr;
// TODO: move some (or all) of these to this crate?
use tezos_api::ffi::{
    ApplyBlockError, ApplyBlockRequest, ApplyBlockResponse, BeginApplicationError,
    BeginApplicationRequest, BeginApplicationResponse, BeginConstructionError,
    BeginConstructionRequest, CommitGenesisResult, ComputePathError, ComputePathRequest,
    ComputePathResponse, FfiJsonEncoderError, GetDataError, HelpersPreapplyBlockRequest,
    HelpersPreapplyError, HelpersPreapplyResponse, InitProtocolContextResult, PrevalidatorWrapper,
    ProtocolDataError, ProtocolRpcError, ProtocolRpcRequest, ProtocolRpcResponse, RustBytes,
    TezosRuntimeConfiguration, TezosRuntimeConfigurationError, TezosStorageInitError,
    ValidateOperationError, ValidateOperationRequest, ValidateOperationResponse,
};
use tezos_context_api::{
    ContextKeyOwned, ContextValue, GenesisChain, PatchContext, ProtocolOverrides, StringTreeObject,
    TezosContextStorageConfiguration,
};
use tezos_messages::p2p::encoding::operation::Operation;

/// This command message is generated by tezedge node and is received by the protocol runner.
#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
pub enum ProtocolMessage {
    ApplyBlockCall(ApplyBlockRequest),
    AssertEncodingForProtocolDataCall(ProtocolHash, RustBytes),
    BeginApplicationCall(BeginApplicationRequest),
    BeginConstructionCall(BeginConstructionRequest),
    ValidateOperationCall(ValidateOperationRequest),
    ProtocolRpcCall(ProtocolRpcRequest),
    HelpersPreapplyOperationsCall(ProtocolRpcRequest),
    HelpersPreapplyBlockCall(HelpersPreapplyBlockRequest),
    ComputePathCall(ComputePathRequest),
    ChangeRuntimeConfigurationCall(TezosRuntimeConfiguration),
    InitProtocolContextCall(InitProtocolContextParams),
    InitProtocolContextIpcServer(TezosContextStorageConfiguration),
    GenesisResultDataCall(GenesisResultDataParams),
    JsonEncodeApplyBlockResultMetadata(JsonEncodeApplyBlockResultMetadataParams),
    JsonEncodeApplyBlockOperationsMetadata(JsonEncodeApplyBlockOperationsMetadataParams),
    ContextGetKeyFromHistory(ContextGetKeyFromHistoryRequest),
    ContextGetKeyValuesByPrefix(ContextGetKeyValuesByPrefixRequest),
    ContextGetTreeByPrefix(ContextGetTreeByPrefixRequest),
    ShutdownCall,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContextGetKeyFromHistoryRequest {
    pub context_hash: ContextHash,
    pub key: ContextKeyOwned,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContextGetKeyValuesByPrefixRequest {
    pub context_hash: ContextHash,
    pub prefix: ContextKeyOwned,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ContextGetTreeByPrefixRequest {
    pub context_hash: ContextHash,
    pub prefix: ContextKeyOwned,
    pub depth: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InitProtocolContextParams {
    pub storage: TezosContextStorageConfiguration,
    pub genesis: GenesisChain,
    pub genesis_max_operations_ttl: u16,
    pub protocol_overrides: ProtocolOverrides,
    pub commit_genesis: bool,
    pub enable_testchain: bool,
    pub readonly: bool,
    pub turn_off_context_raw_inspector: bool,
    pub patch_context: Option<PatchContext>,
    pub context_stats_db_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GenesisResultDataParams {
    pub genesis_context_hash: ContextHash,
    pub chain_id: ChainId,
    pub genesis_protocol_hash: ProtocolHash,
    pub genesis_max_operations_ttl: u16,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonEncodeApplyBlockResultMetadataParams {
    pub context_hash: ContextHash,
    pub metadata_bytes: RustBytes,
    pub max_operations_ttl: i32,
    pub protocol_hash: ProtocolHash,
    pub next_protocol_hash: ProtocolHash,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JsonEncodeApplyBlockOperationsMetadataParams {
    pub chain_id: ChainId,
    pub operations: Vec<Vec<Operation>>,
    pub operations_metadata_bytes: Vec<Vec<RustBytes>>,
    pub protocol_hash: ProtocolHash,
    pub next_protocol_hash: ProtocolHash,
}

/// This event message is generated as a response to the `ProtocolMessage` command.
#[derive(Serialize, Deserialize, Debug, IntoStaticStr)]
pub enum NodeMessage {
    ApplyBlockResult(Result<ApplyBlockResponse, ApplyBlockError>),
    AssertEncodingForProtocolDataResult(Result<(), ProtocolDataError>),
    BeginApplicationResult(Result<BeginApplicationResponse, BeginApplicationError>),
    BeginConstructionResult(Result<PrevalidatorWrapper, BeginConstructionError>),
    ValidateOperationResponse(Result<ValidateOperationResponse, ValidateOperationError>),
    RpcResponse(Result<ProtocolRpcResponse, ProtocolRpcError>),
    HelpersPreapplyResponse(Result<HelpersPreapplyResponse, HelpersPreapplyError>),
    ChangeRuntimeConfigurationResult(Result<(), TezosRuntimeConfigurationError>),
    InitProtocolContextResult(Result<InitProtocolContextResult, TezosStorageInitError>),
    InitProtocolContextIpcServerResult(Result<(), String>), // TODO - TE-261: use actual error result
    CommitGenesisResultData(Result<CommitGenesisResult, GetDataError>),
    ComputePathResponse(Result<ComputePathResponse, ComputePathError>),
    JsonEncodeApplyBlockResultMetadataResponse(Result<String, FfiJsonEncoderError>),
    JsonEncodeApplyBlockOperationsMetadata(Result<String, FfiJsonEncoderError>),
    ContextGetKeyFromHistoryResult(Result<Option<ContextValue>, String>),
    ContextGetKeyValuesByPrefixResult(Result<Option<Vec<(ContextKeyOwned, ContextValue)>>, String>),
    ContextGetTreeByPrefixResult(Result<StringTreeObject, String>),

    ShutdownResult,
}

/// Empty message
#[derive(Serialize, Deserialize, Debug)]
pub struct NoopMessage;