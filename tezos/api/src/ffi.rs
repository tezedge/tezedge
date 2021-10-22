// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt::Debug;
/// Rust implementation of messages required for Rust <-> OCaml FFI communication.
use std::{convert::TryFrom, fmt};

use derive_builder::Builder;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crypto::hash::{
    BlockHash, BlockMetadataHash, ChainId, ContextHash, FromBytesError, OperationHash,
    OperationMetadataHash, OperationMetadataListListHash, ProtocolHash,
};
use tezos_messages::p2p::binary_message::{MessageHash, MessageHashError};
use tezos_messages::p2p::encoding::block_header::{display_fitness, Fitness};
use tezos_messages::p2p::encoding::prelude::{
    BlockHeader, Operation, OperationsForBlocksMessage, Path,
};
use url::Url;

pub mod ffi_error_ids {
    pub const APPLY_ERROR: &str = "ffi.apply_error";
    pub const CALL_ERROR: &str = "ffi.call_error";
    pub const CALL_EXCEPTION: &str = "ffi.call_exception";
    pub const INCONSISTENT_OPERATIONS_HASH: &str = "ffi.inconsistent_operations_hash";
    pub const PREDECESSOR_MISMATCH: &str = "ffi.predecessor_mismatch";
    pub const UNAVAILABLE_PROTOCOL: &str = "ffi.unavailable_protocol";
    pub const UNKNOWN_CONTEXT: &str = "ffi.unknown_context";
    pub const UNKNOWN_PREDECESSOR_CONTEXT: &str = "ffi.unknown_predecessor_context";
}

pub type RustBytes = Vec<u8>;

/// Test chain information
#[derive(Debug, Serialize, Deserialize)]
pub struct TestChain {
    pub chain_id: RustBytes,
    pub protocol_hash: RustBytes,
    pub expiration_date: String,
}

/// Holds configuration for OCaml runtime - e.g. arguments which are passed to OCaml and can be change in runtime
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TezosRuntimeConfiguration {
    pub log_enabled: bool,
    pub debug_mode: bool,
    pub compute_context_action_tree_hashes: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug, Builder)]
pub struct ApplyBlockRequest {
    pub chain_id: ChainId,
    pub block_header: BlockHeader,
    pub pred_header: BlockHeader,

    pub max_operations_ttl: i32,
    pub operations: Vec<Vec<Operation>>,

    pub predecessor_block_metadata_hash: Option<BlockMetadataHash>,
    pub predecessor_ops_metadata_hash: Option<OperationMetadataListListHash>,
}

impl ApplyBlockRequest {
    pub fn convert_operations(
        block_operations: Vec<OperationsForBlocksMessage>,
    ) -> Vec<Vec<Operation>> {
        block_operations.into_iter().map(|ops| ops.into()).collect()
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct CycleRollsOwnerSnapshot {
    pub cycle: i32,
    pub seed_bytes: Vec<u8>,
    pub rolls_data: Vec<(Vec<u8>, Vec<i32>)>,
    pub last_roll: i32,
}

/// Application block result
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ApplyBlockResponse {
    pub validation_result_message: String,
    pub context_hash: ContextHash,
    pub protocol_hash: ProtocolHash,
    pub next_protocol_hash: ProtocolHash,
    pub block_header_proto_json: String,
    pub block_header_proto_metadata_bytes: Vec<u8>,
    pub operations_proto_metadata_bytes: Vec<Vec<Vec<u8>>>,
    pub max_operations_ttl: i32,
    pub last_allowed_fork_level: i32,
    pub forking_testchain: bool,
    pub forking_testchain_data: Option<ForkingTestchainData>,
    pub block_metadata_hash: Option<BlockMetadataHash>,
    pub ops_metadata_hashes: Option<Vec<Vec<OperationMetadataHash>>>,
    // TODO: TE-207 - not needed, can be calculated from ops_metadata_hashes
    /// Note: This is calculated from ops_metadata_hashes - we need this in request
    ///       This is calculated as merkle tree hash, like operation paths
    pub ops_metadata_hash: Option<OperationMetadataListListHash>,
    pub cycle_rolls_owner_snapshots: Vec<CycleRollsOwnerSnapshot>,
    pub new_protocol_constants_json: Option<String>,
    pub new_cycle_eras_json: Option<String>,
    pub commit_time: f64,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct PrevalidatorWrapper {
    pub chain_id: ChainId,
    pub protocol: ProtocolHash,
    pub context_fitness: Option<Fitness>,
}

impl fmt::Debug for PrevalidatorWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "PrevalidatorWrapper[chain_id: {}, protocol: {}, context_fitness: {}]",
            self.chain_id.to_base58_check(),
            self.protocol.to_base58_check(),
            match &self.context_fitness {
                Some(fitness) => display_fitness(fitness),
                None => "-none-".to_string(),
            },
        )
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BeginApplicationRequest {
    pub chain_id: ChainId,
    pub pred_header: BlockHeader,
    pub block_header: BlockHeader,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BeginApplicationResponse {
    pub result: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BeginConstructionRequest {
    pub chain_id: ChainId,
    pub predecessor: BlockHeader,
    pub protocol_data: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ValidateOperationRequest {
    pub prevalidator: PrevalidatorWrapper,
    pub operation: Operation,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ValidateOperationResponse {
    pub prevalidator: PrevalidatorWrapper,
    pub result: ValidateOperationResult,
}

pub type OperationProtocolDataJson = String;
pub type ErrorListJson = String;

#[derive(Serialize, Deserialize)]
pub struct OperationProtocolDataJsonWithErrorListJson {
    pub protocol_data_json: OperationProtocolDataJson,
    pub error_json: ErrorListJson,
}

impl fmt::Debug for OperationProtocolDataJsonWithErrorListJson {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[error_json: {}, protocol_data_json: (-stripped-)]",
            &self.error_json,
        )
    }
}

trait HasOperationHash {
    fn operation_hash(&self) -> &OperationHash;
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Applied {
    pub hash: OperationHash,
    pub protocol_data_json: OperationProtocolDataJson,
}

impl HasOperationHash for Applied {
    fn operation_hash(&self) -> &OperationHash {
        &self.hash
    }
}

impl fmt::Debug for Applied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[hash: {}, protocol_data_json: {}]",
            self.hash.to_base58_check(),
            &self.protocol_data_json
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct Errored {
    pub hash: OperationHash,
    pub is_endorsement: Option<bool>,
    pub protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson,
}

impl HasOperationHash for Errored {
    fn operation_hash(&self) -> &OperationHash {
        &self.hash
    }
}

impl fmt::Debug for Errored {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[hash: {}, protocol_data_json_with_error_json: {:?}]",
            self.hash.to_base58_check(),
            &self.protocol_data_json_with_error_json
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct ValidateOperationResult {
    pub applied: Vec<Applied>,
    pub refused: Vec<Errored>,
    pub branch_refused: Vec<Errored>,
    pub branch_delayed: Vec<Errored>,
    // TODO: outedate?
}

impl ValidateOperationResult {
    /// Merges result with new one, and returns `true/false` if something was changed
    pub fn merge(&mut self, new_result: ValidateOperationResult) -> bool {
        let mut changed = Self::merge_items(&mut self.applied, new_result.applied);
        changed |= Self::merge_items(&mut self.refused, new_result.refused);
        changed |= Self::merge_items(&mut self.branch_refused, new_result.branch_refused);
        changed |= Self::merge_items(&mut self.branch_delayed, new_result.branch_delayed);
        changed
    }

    fn merge_items<ITEM: HasOperationHash>(
        result_items: &mut Vec<ITEM>,
        new_items: Vec<ITEM>,
    ) -> bool {
        let mut changed = false;
        let mut added = false;

        for new_item in new_items {
            // check if present
            let old_value = result_items
                .iter()
                .position(|old_item| old_item.operation_hash().eq(new_item.operation_hash()));

            // replace or add
            if let Some(idx) = old_value {
                // replace
                result_items[idx] = new_item;
                changed |= true;
            } else {
                // add
                result_items.push(new_item);
                added |= true;
            }
        }

        added || changed
    }

    pub fn operations_count(&self) -> usize {
        self.applied.len()
            + self.branch_delayed.len()
            + self.branch_refused.len()
            + self.refused.len()
    }
}

/// Init protocol context result
#[derive(Serialize, Deserialize)]
pub struct InitProtocolContextResult {
    pub supported_protocol_hashes: Vec<ProtocolHash>,
    /// Presents only if was genesis commited to context
    pub genesis_commit_hash: Option<ContextHash>,
}

impl fmt::Debug for InitProtocolContextResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let genesis_commit_hash = match &self.genesis_commit_hash {
            Some(hash) => hash.to_base58_check(),
            None => "-none-".to_string(),
        };
        let supported_protocol_hashes = self
            .supported_protocol_hashes
            .iter()
            .map(|ph| ph.to_base58_check())
            .collect::<Vec<String>>();
        write!(
            f,
            "genesis_commit_hash: {}, supported_protocol_hashes: {:?}",
            &genesis_commit_hash, &supported_protocol_hashes
        )
    }
}

/// Commit genesis result
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CommitGenesisResult {
    pub block_header_proto_json: String,
    pub block_header_proto_metadata_bytes: Vec<u8>,
    pub operations_proto_metadata_bytes: Vec<Vec<Vec<u8>>>,
}

/// Forking test chain data
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct ForkingTestchainData {
    pub forking_block_hash: BlockHash,
    pub test_chain_id: ChainId,
}

/// Represents a trace of errors produced by Tezos.
///
/// `head_error_id` is the id of the main error in the trace, useful for mapping into a Rust error.
/// `trace_json` is json of the trace.
#[derive(Debug)]
pub struct TezosErrorTrace {
    pub head_error_id: String,
    pub trace_json: String,
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum CallError {
    #[error("Failed to call - error_id: {error_id} message: {trace_message:?}!")]
    FailedToCall {
        error_id: String,
        trace_message: String,
    },
    #[error("Invalid request data - message: {message}!")]
    InvalidRequestData { message: String },
    #[error("Invalid response data - message: {message}!")]
    InvalidResponseData { message: String },
}

impl From<TezosErrorTrace> for CallError {
    fn from(error: TezosErrorTrace) -> Self {
        CallError::FailedToCall {
            error_id: error.head_error_id.clone(),
            trace_message: error.trace_json,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum TezosStorageInitError {
    #[error("OCaml storage init failed, message: {message}!")]
    InitializeError { message: String },
}

impl From<TezosErrorTrace> for TezosStorageInitError {
    fn from(error: TezosErrorTrace) -> Self {
        TezosStorageInitError::InitializeError {
            message: error.trace_json,
        }
    }
}

impl From<FromBytesError> for TezosStorageInitError {
    fn from(error: FromBytesError) -> Self {
        TezosStorageInitError::InitializeError {
            message: format!("Error constructing hash from bytes: {:?}", error),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum GetDataError {
    #[error("OCaml failed to get data, message: {message}!")]
    ReadError { message: String },
}

impl From<TezosErrorTrace> for GetDataError {
    fn from(error: TezosErrorTrace) -> Self {
        GetDataError::ReadError {
            message: error.trace_json,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error, PartialEq)]
pub enum ApplyBlockError {
    #[error("Incomplete operations, exptected: {expected}, has actual: {actual}!")]
    IncompleteOperations { expected: usize, actual: usize },
    #[error("Failed to apply block - message: {message}!")]
    FailedToApplyBlock { message: String },
    #[error("Unknown predecessor context - try to apply predecessor at first message: {message}!")]
    UnknownPredecessorContext { message: String },
    #[error("Predecessor does not match - message: {message}!")]
    PredecessorMismatch { message: String },
    #[error("Invalid request/response data - message: {message}!")]
    InvalidRequestResponseData { message: String },
}

impl From<CallError> for ApplyBlockError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::FailedToCall {
                error_id,
                trace_message,
            } => match error_id.as_str() {
                ffi_error_ids::UNKNOWN_PREDECESSOR_CONTEXT => {
                    ApplyBlockError::UnknownPredecessorContext {
                        message: trace_message,
                    }
                }
                ffi_error_ids::PREDECESSOR_MISMATCH => ApplyBlockError::PredecessorMismatch {
                    message: trace_message,
                },
                _ => ApplyBlockError::FailedToApplyBlock {
                    message: trace_message,
                },
            },
            CallError::InvalidRequestData { message } => {
                ApplyBlockError::InvalidRequestResponseData { message }
            }
            CallError::InvalidResponseData { message } => {
                ApplyBlockError::InvalidRequestResponseData { message }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum BeginApplicationError {
    #[error("Failed to begin application - message: {message}!")]
    FailedToBeginApplication { message: String },
    #[error("Unknown predecessor context - try to apply predecessor at first message: {message}!")]
    UnknownPredecessorContext { message: String },
    #[error("Invalid request/response data - message: {message}!")]
    InvalidRequestResponseData { message: String },
}

impl From<CallError> for BeginApplicationError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::FailedToCall {
                error_id,
                trace_message,
            } => match error_id.as_str() {
                ffi_error_ids::UNKNOWN_PREDECESSOR_CONTEXT => {
                    BeginApplicationError::UnknownPredecessorContext {
                        message: trace_message,
                    }
                }
                _ => BeginApplicationError::FailedToBeginApplication {
                    message: trace_message,
                },
            },
            CallError::InvalidRequestData { message } => {
                BeginApplicationError::InvalidRequestResponseData { message }
            }
            CallError::InvalidResponseData { message } => {
                BeginApplicationError::InvalidRequestResponseData { message }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum BeginConstructionError {
    #[error("Failed to begin construction - message: {message}!")]
    FailedToBeginConstruction { message: String },
    #[error("Unknown predecessor context - try to apply predecessor at first message: {message}!")]
    UnknownPredecessorContext { message: String },
    #[error("Invalid request/response data - message: {message}!")]
    InvalidRequestResponseData { message: String },
}

impl From<CallError> for BeginConstructionError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::FailedToCall {
                error_id,
                trace_message,
            } => match error_id.as_str() {
                ffi_error_ids::UNKNOWN_PREDECESSOR_CONTEXT => {
                    BeginConstructionError::UnknownPredecessorContext {
                        message: trace_message,
                    }
                }
                _ => BeginConstructionError::FailedToBeginConstruction {
                    message: trace_message,
                },
            },
            CallError::InvalidRequestData { message } => {
                BeginConstructionError::InvalidRequestResponseData { message }
            }
            CallError::InvalidResponseData { message } => {
                BeginConstructionError::InvalidRequestResponseData { message }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum ValidateOperationError {
    #[error("Failed to validate operation - message: {message}!")]
    FailedToValidateOperation { message: String },
    #[error("Invalid request/response data - message: {message}!")]
    InvalidRequestResponseData { message: String },
}

impl From<CallError> for ValidateOperationError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::FailedToCall { trace_message, .. } => {
                ValidateOperationError::FailedToValidateOperation {
                    message: trace_message,
                }
            }
            CallError::InvalidRequestData { message } => {
                ValidateOperationError::InvalidRequestResponseData { message }
            }
            CallError::InvalidResponseData { message } => {
                ValidateOperationError::InvalidRequestResponseData { message }
            }
        }
    }
}

// NOTE: used by decode_context_data, which is unused at the moment
#[derive(Debug, Error)]
pub enum ContextDataError {
    #[error("Resolve/decode context data failed to decode: {message}!")]
    DecodeError { message: String },
}

impl From<TezosErrorTrace> for ContextDataError {
    fn from(error: TezosErrorTrace) -> Self {
        ContextDataError::DecodeError {
            message: error.trace_json,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum ProtocolDataError {
    #[error("Resolve/decode context data failed to decode: {message}!")]
    DecodeError { message: String },
}

impl From<TezosErrorTrace> for ProtocolDataError {
    fn from(error: TezosErrorTrace) -> Self {
        ProtocolDataError::DecodeError {
            message: error.trace_json,
        }
    }
}

impl From<FromBytesError> for ProtocolDataError {
    fn from(error: FromBytesError) -> Self {
        ProtocolDataError::DecodeError {
            message: format!("Error constructing hash from bytes: {:?}", error),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum FfiJsonEncoderError {
    #[error("FFI JSON encoding error: {message}!")]
    EncodeError { message: String },
}

impl From<TezosErrorTrace> for FfiJsonEncoderError {
    fn from(error: TezosErrorTrace) -> Self {
        FfiJsonEncoderError::EncodeError {
            message: error.trace_json,
        }
    }
}

pub type Json = String;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RpcRequest {
    pub body: Json,
    pub context_path: String,
    pub meth: RpcMethod,
    pub content_type: Option<String>,
    pub accept: Option<String>,
}

impl RpcRequest {
    /// Produces a key for the routed requests cache.
    ///
    /// The cache key is just the original path+query, with a bit of normalization
    /// and the "/chains/:chan_id/blocks/:block_id" prefix removed if present.
    pub fn ffi_rpc_router_cache_key(&self) -> String {
        Self::ffi_rpc_router_cache_key_helper(&self.context_path)
    }

    fn ffi_rpc_router_cache_key_helper(path: &str) -> String {
        let base = match Url::parse("http://tezedge.com") {
            Ok(base) => base,
            Err(_) => return path.to_string(),
        };
        let parsed = Url::options().base_url(Some(&base)).parse(path).unwrap();
        let normalized_path = match parsed.query() {
            Some(query) => format!("{}?{}", parsed.path().trim_end_matches('/'), query),
            None => parsed.path().trim_end_matches('/').to_string(),
        };
        let mut segments = match parsed.path_segments() {
            Some(segments) => segments,
            // Not the subpath we expect, bail-out
            None => return normalized_path,
        };

        // /chains/:chain_id/blocks/:block_id
        let (chains, _, blocks, _) = (
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next(),
        );

        match (chains, blocks) {
            (Some("chains"), Some("blocks")) => (),
            // Not the subpath we expect, bail-out
            _ => return normalized_path,
        }

        let remaining: Vec<_> = segments.filter(|s| !s.is_empty()).collect();
        let subpath = remaining.join("/");

        // We only care about subpaths, bail-out
        if subpath.is_empty() {
            return normalized_path;
        }

        if let Some(query) = parsed.query() {
            format!("/{}?{}", subpath, query)
        } else {
            format!("/{}", subpath)
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HelpersPreapplyBlockRequest {
    pub protocol_rpc_request: ProtocolRpcRequest,
    pub predecessor_block_metadata_hash: Option<BlockMetadataHash>,
    pub predecessor_ops_metadata_hash: Option<OperationMetadataListListHash>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct HelpersPreapplyResponse {
    pub body: Json,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ProtocolRpcResponse {
    RPCConflict(Option<String>),
    RPCCreated(Option<String>),
    RPCError(Option<String>),
    RPCForbidden(Option<String>),
    RPCGone(Option<String>),
    RPCNoContent,
    RPCNotFound(Option<String>),
    RPCOk(String),
    RPCUnauthorized,
}

fn body_or_empty(body: &Option<String>) -> String {
    match body {
        Some(body) => body.clone(),
        None => "".to_string(),
    }
}

impl ProtocolRpcResponse {
    pub fn status_code(&self) -> u16 {
        // These HTTP codes are mapped form what the `resto` OCaml library defines
        match self {
            ProtocolRpcResponse::RPCConflict(_) => 409,
            ProtocolRpcResponse::RPCCreated(_) => 201,
            ProtocolRpcResponse::RPCError(_) => 500,
            ProtocolRpcResponse::RPCForbidden(_) => 403,
            ProtocolRpcResponse::RPCGone(_) => 410,
            ProtocolRpcResponse::RPCNoContent => 204,
            ProtocolRpcResponse::RPCNotFound(_) => 404,
            ProtocolRpcResponse::RPCOk(_) => 200,
            ProtocolRpcResponse::RPCUnauthorized => 401,
        }
    }

    pub fn body_json_string_or_empty(&self) -> String {
        match self {
            ProtocolRpcResponse::RPCConflict(body) => body_or_empty(body),
            ProtocolRpcResponse::RPCCreated(body) => body_or_empty(body),
            ProtocolRpcResponse::RPCError(body) => body_or_empty(body),
            ProtocolRpcResponse::RPCForbidden(body) => body_or_empty(body),
            ProtocolRpcResponse::RPCGone(body) => body_or_empty(body),
            ProtocolRpcResponse::RPCNoContent => "".to_string(),
            ProtocolRpcResponse::RPCNotFound(body) => body_or_empty(body),
            ProtocolRpcResponse::RPCOk(body) => body.clone(),
            ProtocolRpcResponse::RPCUnauthorized => "".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RpcMethod {
    DELETE,
    GET,
    PATCH,
    POST,
    PUT,
}

impl TryFrom<&str> for RpcMethod {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let upper = s.to_uppercase();
        match upper.as_ref() {
            "DELETE" => Ok(RpcMethod::DELETE),
            "GET" => Ok(RpcMethod::GET),
            "PATCH" => Ok(RpcMethod::PATCH),
            "POST" => Ok(RpcMethod::POST),
            "PUT" => Ok(RpcMethod::PUT),
            other => Err(format!("Invalid RPC method: {:?}", other)),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RpcArgDesc {
    pub name: String,
    pub descr: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum ProtocolRpcError {
    #[error("RPC: cannot parse body: {0}")]
    RPCErrorCannotParseBody(String),
    #[error("RPC: cannot parse path: {0:?}, arg_desc={1:?}, message: {2}")]
    RPCErrorCannotParsePath(Vec<String>, RpcArgDesc, String),
    #[error("RPC: cannot parse query: {0}")]
    RPCErrorCannotParseQuery(String),
    #[error("RPC: invalid method string: {0}")]
    RPCErrorInvalidMethodString(String),
    #[error("RPC: method not allowed: {0:?}")]
    RPCErrorMethodNotAllowed(Vec<RpcMethod>),
    #[error("RPC: service not found")]
    RPCErrorServiceNotFound,
    #[error("RPC: Failed to call protocol RPC - message: {0}!")]
    FailedToCallProtocolRpc(String),
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ProtocolRpcRequest {
    pub block_header: BlockHeader,
    pub chain_arg: String,
    pub chain_id: ChainId,

    pub request: RpcRequest,
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum HelpersPreapplyError {
    #[error("Failed to call protocol rpc - message: {message}!")]
    FailedToCallProtocolRpc { message: String },
    #[error("Invalid request data - message: {message}!")]
    InvalidRequestData { message: String },
    #[error("Invalid response data - message: {message}!")]
    InvalidResponseData { message: String },
}

impl From<CallError> for HelpersPreapplyError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::FailedToCall { trace_message, .. } => {
                HelpersPreapplyError::FailedToCallProtocolRpc {
                    message: trace_message,
                }
            }
            CallError::InvalidRequestData { message } => {
                HelpersPreapplyError::InvalidRequestData { message }
            }
            CallError::InvalidResponseData { message } => {
                HelpersPreapplyError::InvalidResponseData { message }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ComputePathRequest {
    pub operations: Vec<Vec<OperationHash>>,
}

impl TryFrom<&Vec<Vec<Operation>>> for ComputePathRequest {
    type Error = MessageHashError;

    fn try_from(ops: &Vec<Vec<Operation>>) -> Result<Self, Self::Error> {
        let mut operation_hashes = Vec::with_capacity(ops.len());
        for inner_ops in ops {
            let mut iophs = Vec::with_capacity(inner_ops.len());
            for op in inner_ops {
                iophs.push(OperationHash::try_from(op.message_hash()?)?);
            }
            operation_hashes.push(iophs);
        }

        Ok(ComputePathRequest {
            operations: operation_hashes,
        })
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ComputePathResponse {
    pub operations_hashes_path: Vec<Path>,
}

#[derive(Serialize, Deserialize, Debug, Error)]
pub enum ComputePathError {
    #[error("Path computation failed, message: {message}!")]
    PathError { message: String },
    #[error("Path computation failed, message: {message}!")]
    InvalidRequestResponseData { message: String },
}

impl From<CallError> for ComputePathError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::FailedToCall { trace_message, .. } => ComputePathError::PathError {
                message: trace_message,
            },
            CallError::InvalidRequestData { message } => {
                ComputePathError::InvalidRequestResponseData { message }
            }
            CallError::InvalidResponseData { message } => {
                ComputePathError::InvalidRequestResponseData { message }
            }
        }
    }
}

/// Error types generated by a tezos protocol.
#[derive(Error, Debug)]
pub enum ProtocolError {
    /// Protocol rejected to apply a block.
    #[error("Apply block error: {reason}")]
    ApplyBlockError { reason: ApplyBlockError },
    #[error("Assert encoding for protocol data error: {reason}")]
    AssertEncodingForProtocolDataError { reason: ProtocolDataError },
    #[error("Begin construction error: {reason}")]
    BeginApplicationError { reason: BeginApplicationError },
    #[error("Begin construction error: {reason}")]
    BeginConstructionError { reason: BeginConstructionError },
    #[error("Validate operation error: {reason}")]
    ValidateOperationError { reason: ValidateOperationError },
    #[error("Protocol rpc call error: {reason}")]
    ProtocolRpcError {
        reason: ProtocolRpcError,
        request_path: String,
    },
    #[error("Helper Preapply call error: {reason}")]
    HelpersPreapplyError { reason: HelpersPreapplyError },
    #[error("Compute path call error: {reason}")]
    ComputePathError { reason: ComputePathError },
    /// OCaml part failed to initialize tezos storage.
    #[error("OCaml storage init error: {reason}")]
    OcamlStorageInitError { reason: TezosStorageInitError },
    /// OCaml part failed to get genesis data.
    #[error("Failed to get genesis data: {reason}")]
    GenesisResultDataError { reason: GetDataError },
    #[error("Failed to decode binary data to json ({caller}): {reason}")]
    FfiJsonEncoderError {
        caller: String,
        reason: FfiJsonEncoderError,
    },

    #[error("Failed to get key from history: {reason}")]
    ContextGetKeyFromHistoryError { reason: String },
    #[error("Failed to get values by prefix: {reason}")]
    ContextGetKeyValuesByPrefixError { reason: String },
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use assert_json_diff::assert_json_eq;
    use tezos_context_api::ProtocolOverrides;

    use super::*;

    #[test]
    fn test_validate_operation_result_merge() {
        let mut validate_result1 = validate_operation_result(
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ",
            "opVUxMhZttd858HXEHCgchknnnZFmUExtHrbmVSh1G9Pg24X1Pj",
        );
        assert_eq!(2, validate_result1.applied.len());
        assert_eq!(2, validate_result1.refused.len());
        assert_eq!(2, validate_result1.branch_delayed.len());
        assert_eq!(2, validate_result1.branch_refused.len());

        // merge empty -> no change
        assert_eq!(
            false,
            validate_result1.merge(ValidateOperationResult {
                applied: vec![],
                refused: vec![],
                branch_refused: vec![],
                branch_delayed: vec![],
            })
        );
        assert_eq!(2, validate_result1.applied.len());
        assert_eq!(2, validate_result1.refused.len());
        assert_eq!(2, validate_result1.branch_delayed.len());
        assert_eq!(2, validate_result1.branch_refused.len());

        // merge
        let validate_result2 = validate_operation_result(
            "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ",
            "opJ4FdKumPfykAP9ZqwY7rNB8y1SiMupt44RqBDMWL7cmb4xbNr",
        );
        assert!(validate_result1.merge(validate_result2));
        assert_eq!(3, validate_result1.applied.len());
        assert_eq!(3, validate_result1.refused.len());
        assert_eq!(3, validate_result1.branch_delayed.len());
        assert_eq!(3, validate_result1.branch_refused.len());
    }

    #[test]
    fn test_validate_operation_result_merge_items() -> Result<(), anyhow::Error> {
        let mut validate_result = ValidateOperationResult {
            applied: vec![],
            refused: vec![],
            branch_refused: vec![],
            branch_delayed: vec![],
        };
        assert_eq!(0, validate_result.applied.len());
        assert_eq!(
            false,
            ValidateOperationResult::merge_items(&mut validate_result.applied, vec![])
        );

        assert!(ValidateOperationResult::merge_items(
            &mut validate_result.applied,
            vec![Applied {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
                protocol_data_json: "protocol_data_json1".to_string(),
            },],
        ));
        assert_eq!(1, validate_result.applied.len());
        assert_eq!(
            "protocol_data_json1",
            validate_result.applied[0].protocol_data_json
        );

        // merge the same -> test change
        assert!(ValidateOperationResult::merge_items(
            &mut validate_result.applied,
            vec![Applied {
                hash: "onvN8U6QJ6DGJKVYkHXYRtFm3tgBJScj9P5bbPjSZUuFaGzwFuJ".try_into()?,
                protocol_data_json: "protocol_data_json2".to_string(),
            },],
        ));
        assert_eq!(1, validate_result.applied.len());
        assert_eq!(
            "protocol_data_json2",
            validate_result.applied[0].protocol_data_json
        );

        // merge another new one
        assert!(ValidateOperationResult::merge_items(
            &mut validate_result.applied,
            vec![Applied {
                hash: "opJ4FdKumPfykAP9ZqwY7rNB8y1SiMupt44RqBDMWL7cmb4xbNr".try_into()?,
                protocol_data_json: "protocol_data_json2".to_string(),
            },],
        ));
        assert_eq!(2, validate_result.applied.len());

        Ok(())
    }

    fn validate_operation_result(op1: &str, op2: &str) -> ValidateOperationResult {
        let applied = vec![
            Applied {
                hash: op1.try_into().expect("Error"),
                protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
            },
            Applied {
                hash: op2.try_into().expect("Error"),
                protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
            }
        ];

        let branch_delayed = vec![
            Errored {
                hash: op1.try_into().expect("Error"),
                is_endorsement: None,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            },
            Errored {
                hash: op2.try_into().expect("Error"),
                is_endorsement: None,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            }
        ];

        let branch_refused = vec![
            Errored {
                hash: op1.try_into().expect("Error"),
                is_endorsement: None,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            },
            Errored {
                hash: op2.try_into().expect("Error"),
                is_endorsement: None,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            }
        ];

        let refused = vec![
            Errored {
                hash: op1.try_into().expect("Error"),
                is_endorsement: None,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            },
            Errored {
                hash: op2.try_into().expect("Error"),
                is_endorsement: None,
                protocol_data_json_with_error_json: OperationProtocolDataJsonWithErrorListJson {
                    protocol_data_json: "{ \"contents\": [ { \"kind\": \"endorsement\", \"level\": 459020 } ],\n  \"signature\":\n    \"siguKbKFVDkXo2m1DqZyftSGg7GZRq43EVLSutfX5yRLXXfWYG5fegXsDT6EUUqawYpjYE1GkyCVHfc2kr3hcaDAvWSAhnV9\" }".to_string(),
                    error_json: "[ { \"kind\": \"temporary\",\n    \"id\": \"proto.005-PsBabyM1.operation.wrong_endorsement_predecessor\",\n    \"expected\": \"BMDb9PfcJmiibDDEbd6bEEDj4XNG4C7QACG6TWqz29c9FxNgDLL\",\n    \"provided\": \"BLd8dLs4X5Ve6a8B37kUu7iJkRycWzfSF5MrskY4z8YaideQAp4\" } ]".to_string(),
                },
            }
        ];

        ValidateOperationResult {
            applied,
            branch_delayed,
            branch_refused,
            refused,
        }
    }

    #[test]
    fn test_rpc_format_user_activated_upgrades() -> Result<(), anyhow::Error> {
        let expected_json = serde_json::json!(
            [
              {
                "level": 28082,
                "replacement_protocol": "PsYLVpVvgbLhAhoqAkMFUo6gudkJ9weNXhUYCiLDzcUpFpkk8Wt"
              },
              {
                "level": 204761,
                "replacement_protocol": "PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP"
              }
            ]
        );

        let protocol_overrides = ProtocolOverrides {
            user_activated_upgrades: vec![
                (
                    28082_i32,
                    "PsYLVpVvgbLhAhoqAkMFUo6gudkJ9weNXhUYCiLDzcUpFpkk8Wt".to_string(),
                ),
                (
                    204761_i32,
                    "PsddFKi32cMJ2qPjf43Qv5GDWLDPZb3T3bF6fLKiF5HtvHNU7aP".to_string(),
                ),
            ],
            user_activated_protocol_overrides: vec![],
        };

        assert_json_eq!(
            expected_json,
            serde_json::to_value(protocol_overrides.user_activated_upgrades_to_rpc_json())?
        );
        Ok(())
    }

    #[test]
    fn test_rpc_format_user_activated_protocol_overrides() -> Result<(), anyhow::Error> {
        let expected_json = serde_json::json!(
            [
              {
                "replaced_protocol": "PsBABY5HQTSkA4297zNHfsZNKtxULfL18y95qb3m53QJiXGmrbU",
                "replacement_protocol": "PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS"
              }
            ]
        );

        let protocol_overrides = ProtocolOverrides {
            user_activated_upgrades: vec![],
            user_activated_protocol_overrides: vec![(
                "PsBABY5HQTSkA4297zNHfsZNKtxULfL18y95qb3m53QJiXGmrbU".to_string(),
                "PsBabyM1eUXZseaJdmXFApDSBqj8YBfwELoxZHHW77EMcAbbwAS".to_string(),
            )],
        };

        assert_json_eq!(
            expected_json,
            serde_json::to_value(
                protocol_overrides.user_activated_protocol_overrides_to_rpc_json()
            )?
        );
        Ok(())
    }

    #[test]
    fn test_rpc_ffi_rpc_router_cache_key_helper() {
        let with_prefix_to_remove = "/chains/main/blocks/head/some/subpath/url";
        assert_eq!(
            "/some/subpath/url".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(with_prefix_to_remove)
        );

        let without_prefix_to_remove = "/chains/main/something/else/some/subpath/url";
        assert_eq!(
            without_prefix_to_remove.to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(without_prefix_to_remove)
        );

        let without_suffix = "/chains/main/blocks/head/";
        assert_eq!(
            "/chains/main/blocks/head".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(without_suffix)
        );

        let without_prefix_to_remove_short = "/chains/main/";
        assert_eq!(
            "/chains/main".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(without_prefix_to_remove_short)
        );

        let with_prefix_to_remove_and_query =
            "/chains/main/blocks/head/some/subpath/url?query=args&with-slash=/slash";
        assert_eq!(
            "/some/subpath/url?query=args&with-slash=/slash".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(with_prefix_to_remove_and_query)
        );

        let without_suffix_and_query = "/chains/main/blocks/head/?query=1";
        assert_eq!(
            "/chains/main/blocks/head?query=1".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(without_suffix_and_query)
        );

        let without_suffix_and_slashes = "/chains/main/blocks/head//";
        assert_eq!(
            "/chains/main/blocks/head".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(without_suffix_and_slashes)
        );

        let without_suffix_and_sharp = "/chains/main/blocks/head/#";
        assert_eq!(
            "/chains/main/blocks/head".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(without_suffix_and_sharp)
        );

        let with_prefix_to_remove_with_question_mark = "/chains/main?/blocks/head/some/subpath/url";
        assert_eq!(
            with_prefix_to_remove_with_question_mark.to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(with_prefix_to_remove_with_question_mark)
        );

        let with_prefix_to_remove_with_sharp = "/chains/main#/blocks/head/some/subpath/url";
        assert_eq!(
            "/chains/main".to_string(),
            RpcRequest::ffi_rpc_router_cache_key_helper(with_prefix_to_remove_with_sharp)
        );
    }
}
