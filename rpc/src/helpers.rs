// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::PoisonError;
use std::{collections::HashMap, convert::TryFrom};
use std::{convert::TryInto, ops::Neg};

use anyhow::bail;
use async_ipc::IpcError;
use hex::FromHexError;
use hyper::{Body, Request};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId, ProtocolHash};
use shell_integration::Prevalidator;
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::{
    BlockAdditionalData, BlockHeaderWithHash, BlockJsonData, BlockMetaStorage,
    BlockMetaStorageReader, BlockStorage, BlockStorageReader, ChainMetaStorage, StorageError,
};
use tezos_api::ffi::{RpcMethod, RpcRequest};
use tezos_messages::p2p::binary_message::MessageHashError;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::prelude::*;
use tezos_messages::{ts_to_rfc3339, TimestampOutOfRangeError};

use crate::encoding::base_types::UniString;
use crate::server::{HasSingleValue, Query, RpcServiceEnvironment};

#[macro_export]
macro_rules! merge_slices {
    ( $($x:expr),* ) => {{
        let mut res = vec![];
        $(
            res.extend_from_slice($x);
        )*
        res
    }}
}

#[macro_export]
macro_rules! required_param {
    ($params:expr, $param_name:expr) => {{
        match $params.get_str($param_name) {
            Some(param_value) => Ok(param_value),
            None => Err(anyhow::format_err!("Missing parameter '{}'", $param_name)),
        }
    }};
}

#[macro_export]
macro_rules! parse_block_hash_or_fail {
    ($chain_id:expr, $block_id_param:expr, $env:expr) => {{
        match parse_block_hash($chain_id, $block_id_param, $env) {
            Ok(block_hash) => block_hash,
            Err(crate::helpers::RpcServiceError::NoDataFoundError { .. }) => {
                return crate::not_found();
            }
            Err(e) => return crate::error(anyhow::format_err!("{}", e)),
        }
    }};
}

/// Possible errors for state processing
#[derive(Debug, Error)]
pub enum RpcServiceError {
    #[error("Storage read error, reason: {error:?}")]
    StorageError {
        #[from]
        error: StorageError,
    },
    #[error("IPC error: {reason}")]
    IpcError {
        #[from]
        reason: IpcError,
    },
    #[error("No data found error, reason: {reason:?}")]
    NoDataFoundError { reason: String },
    #[error("Invalid parameters, reason: {reason:?}")]
    InvalidParameters { reason: String },
    #[error("Unexpected/unhandled error occurred, reason: {reason:?}")]
    UnexpectedError { reason: String },
}

impl From<FromHexError> for RpcServiceError {
    fn from(error: FromHexError) -> Self {
        Self::UnexpectedError {
            reason: format!("{}", error),
        }
    }
}

impl From<MessageHashError> for RpcServiceError {
    fn from(error: MessageHashError) -> Self {
        Self::UnexpectedError {
            reason: format!("{}", error),
        }
    }
}

impl From<serde_json::Error> for RpcServiceError {
    fn from(error: serde_json::Error) -> Self {
        Self::UnexpectedError {
            reason: format!("{}", error),
        }
    }
}

impl<T> From<PoisonError<T>> for RpcServiceError {
    fn from(pe: PoisonError<T>) -> Self {
        RpcServiceError::UnexpectedError {
            reason: format!("{}", pe),
        }
    }
}

impl From<TimestampOutOfRangeError> for RpcServiceError {
    fn from(error: TimestampOutOfRangeError) -> Self {
        RpcServiceError::UnexpectedError {
            reason: format!("{}", error),
        }
    }
}

pub type BlockHeaderJson = HashMap<String, Value>;
pub type BlockMetadata = HashMap<String, Value>;
pub type BlockOperations = Vec<BlockValidationPass>;
pub type BlockValidationPass = Vec<BlockOperation>;
pub type BlockOperation = HashMap<String, Value>;

#[derive(Serialize, Debug, Clone)]
pub struct BlockInfo {
    protocol: String,
    chain_id: String,
    hash: String,
    header: BlockHeaderJson,
    metadata: BlockMetadata,
    operations: BlockOperations,
}

impl BlockInfo {
    pub fn new(
        chain_id: &ChainId,
        block_hash: &BlockHash,
        protocol: ProtocolHash,
        header: InnerBlockHeader,
        metadata: BlockMetadata,
        operations: BlockOperations,
    ) -> Self {
        Self {
            protocol: protocol.to_base58_check(),
            chain_id: chain_id.to_base58_check(),
            hash: block_hash.to_base58_check(),
            header: header.into(),
            metadata,
            operations,
        }
    }
}

/// Object containing all block header information
#[derive(Serialize, Debug, Clone)]
pub struct InnerBlockHeader {
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
    pub protocol_data: HashMap<String, Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct HeaderContent {
    pub command: String,
    pub hash: String,
    pub fitness: Vec<String>,
    pub protocol_parameters: String,
}

/// Object containing information to recreate the block header information
#[derive(Serialize, Debug, Clone)]
pub struct BlockHeaderInfo {
    pub hash: String,
    pub chain_id: String,
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
    pub protocol: String,

    // TODO: refactor this to support multiple protocol version encoding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proof_of_work_nonce: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub liquidity_baking_escape_vote: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<HeaderContent>,
}

/// Object containing information to recreate the block header shell information
#[derive(Serialize, Debug, Clone)]
pub struct BlockHeaderShellInfo {
    pub level: i32,
    pub proto: u8,
    pub predecessor: String,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: String,
    pub fitness: Vec<String>,
    pub context: String,
}

impl BlockHeaderShellInfo {
    pub fn try_new(block: &BlockHeaderWithHash) -> Result<Self, TimestampOutOfRangeError> {
        Ok(BlockHeaderShellInfo {
            level: block.header.level(),
            proto: block.header.proto(),
            predecessor: block.header.predecessor().to_base58_check(),
            timestamp: ts_to_rfc3339(block.header.timestamp())?,
            validation_pass: block.header.validation_pass(),
            operations_hash: block.header.operations_hash().to_base58_check(),
            fitness: block
                .header
                .fitness()
                .iter()
                .map(|x| hex::encode(&x))
                .collect(),
            context: block.header.context().to_base58_check(),
        })
    }
}

impl BlockHeaderInfo {
    pub fn try_new(
        block: &BlockHeaderWithHash,
        block_json_data: &BlockJsonData,
        block_additional_data: &BlockAdditionalData,
        chain_id: &ChainId,
    ) -> Result<Self, TimestampOutOfRangeError> {
        let header: &BlockHeader = &block.header;
        let predecessor = header.predecessor().to_base58_check();
        let timestamp = ts_to_rfc3339(header.timestamp())?;
        let operations_hash = header.operations_hash().to_base58_check();
        let fitness = header.fitness().iter().map(|x| hex::encode(&x)).collect();
        let context = header.context().to_base58_check();
        let hash = block.hash.to_base58_check();

        let header_data: HashMap<String, Value> =
            serde_json::from_str(block_json_data.block_header_proto_json()).unwrap_or_default();
        let signature = header_data
            .get("signature")
            .map(|val| val.as_str().unwrap().to_string());
        let priority = header_data.get("priority").map(|val| val.as_i64().unwrap());
        let proof_of_work_nonce = header_data
            .get("proof_of_work_nonce")
            .map(|val| val.as_str().unwrap().to_string());
        let seed_nonce_hash = header_data
            .get("seed_nonce_hash")
            .map(|val| val.as_str().unwrap().to_string());
        let liquidity_baking_escape_vote = header_data
            .get("liquidity_baking_escape_vote")
            .map(|val| val.as_bool().unwrap());

        let mut content: Option<HeaderContent> = None;
        if let Some(header_content) = header_data.get("content") {
            content = serde_json::from_value(header_content.clone()).unwrap();
        }

        Ok(Self {
            hash,
            chain_id: chain_id.to_base58_check(),
            level: header.level(),
            proto: header.proto(),
            predecessor,
            timestamp,
            validation_pass: header.validation_pass(),
            operations_hash,
            fitness,
            context,
            protocol: block_additional_data.protocol_hash().to_base58_check(),
            signature,
            priority,
            seed_nonce_hash,
            proof_of_work_nonce,
            liquidity_baking_escape_vote,
            content,
        })
    }
}

impl From<InnerBlockHeader> for HashMap<String, Value> {
    fn from(inner_block_header: InnerBlockHeader) -> Self {
        let mut map: HashMap<String, Value> = HashMap::new();
        map.insert("level".to_string(), inner_block_header.level.into());
        map.insert("proto".to_string(), inner_block_header.proto.into());
        map.insert(
            "predecessor".to_string(),
            inner_block_header.predecessor.into(),
        );
        map.insert("timestamp".to_string(), inner_block_header.timestamp.into());
        map.insert(
            "validation_pass".to_string(),
            inner_block_header.validation_pass.into(),
        );
        map.insert(
            "operations_hash".to_string(),
            inner_block_header.operations_hash.into(),
        );
        map.insert("fitness".to_string(), inner_block_header.fitness.into());
        map.insert("context".to_string(), inner_block_header.context.into());
        map.extend(inner_block_header.protocol_data);
        map
    }
}

/// Represents generic paged result.
#[derive(Debug, Serialize)]
pub struct PagedResult<C: Serialize> {
    /// Paged result data.
    data: C,
    /// ID of the next item if more items are available.
    /// If no more items are available then `None`.
    next_id: Option<u64>,
    /// Limit used in the request which produced this paged result.
    limit: usize,
}

#[allow(dead_code)]
impl<C> PagedResult<C>
where
    C: Serialize,
{
    pub fn new(data: C, next_id: Option<u64>, limit: usize) -> Self {
        PagedResult {
            data,
            next_id,
            limit,
        }
    }
}

#[derive(Serialize, Debug, Clone)]
pub struct Protocols {
    protocol: String,
    next_protocol: String,
}

impl Protocols {
    pub fn new(protocol: String, next_protocol: String) -> Self {
        Self {
            protocol,
            next_protocol,
        }
    }
}

// ---------------------------------------------------------------------
#[derive(Serialize, Debug, Clone)]
pub struct NodeVersion {
    version: Version,
    network_version: NetworkVersion,
    commit_info: CommitInfo,
}

#[derive(Serialize, Debug, Clone)]
pub struct CommitInfo {
    commit_hash: UniString,
    commit_date: UniString,
}

#[derive(Serialize, Debug, Clone)]
pub struct Version {
    major: i32,
    minor: i32,
    additional_info: String,
}

impl NodeVersion {
    pub fn new(network_version: &NetworkVersion) -> Self {
        let version_env: &'static str = env!("CARGO_PKG_VERSION");

        let version: Vec<String> = version_env.split('.').map(|v| v.to_string()).collect();

        Self {
            version: Version {
                major: version[0].parse().unwrap_or(0),
                minor: version[1].parse().unwrap_or(0),
                additional_info: "release".to_string(),
            },
            network_version: network_version.clone(),
            commit_info: CommitInfo {
                commit_hash: UniString::from(env!("GIT_HASH")),
                commit_date: UniString::from(env!("GIT_COMMIT_DATE")),
            },
        }
    }
}

pub const MAIN_CHAIN_ID: &str = "main";
pub const TEST_CHAIN_ID: &str = "test";

/// Parses [ChainId] from chain_id url param
pub(crate) fn parse_chain_id(
    chain_id_param: &str,
    env: &RpcServiceEnvironment,
) -> Result<ChainId, anyhow::Error> {
    Ok(match chain_id_param {
        MAIN_CHAIN_ID => env.main_chain_id().clone(),
        TEST_CHAIN_ID => {
            // find test chain for main chain
            let chain_meta_storage = ChainMetaStorage::new(env.persistent_storage());
            let test_chain = match chain_meta_storage.get_test_chain_id(env.main_chain_id())? {
                Some(test_chain_id) => test_chain_id,
                None => bail!(
                    "No test chain activated for main_chain_id: {}",
                    env.main_chain_id().to_base58_check()
                ),
            };

            bail!(
                "Test chains are not supported yet! main_chain_id: {}, test_chain_id: {}",
                env.main_chain_id().to_base58_check(),
                test_chain.to_base58_check()
            )
        }
        chain_id_hash => {
            let chain_id: ChainId = chain_id_hash.try_into()?;
            if chain_id.eq(env.main_chain_id()) {
                chain_id
            } else {
                bail!("Multiple chains are not supported yet! requested_chain_id: {} only main_chain_id: {}",
                        chain_id.to_base58_check(),
                        env.main_chain_id().to_base58_check())
            }
        }
    })
}

/// Parses [async] parameter from query
pub(crate) fn parse_async(query: &Query, default: bool) -> bool {
    match query.get_str("async") {
        Some(value) => value.eq("true"),
        None => default,
    }
}

fn split_block_id_param(
    block_id_param: &str,
    split_char: char,
    negate: bool,
) -> Result<(&str, Option<i32>), RpcServiceError> {
    let splits: Vec<&str> = block_id_param.split(split_char).collect();
    match splits.len() {
        1 => Ok((splits[0], None)),
        2 => {
            // handles cases like /chains/main/blocks/head~, where '~' is included without a value
            if splits[1].is_empty() {
                Ok((splits[0], Some(0)))
            } else {
                let offset =
                    splits[1]
                        .parse::<i32>()
                        .map_err(|e| RpcServiceError::InvalidParameters {
                            reason: format!(
                                "Invalid offset part of block_id_param: {}, reason: {}",
                                block_id_param, e
                            ),
                        })?;
                if negate {
                    Ok((splits[0], Some(offset.neg())))
                } else {
                    Ok((splits[0], Some(offset)))
                }
            }
        }
        _ => Err(RpcServiceError::InvalidParameters {
            reason: format!("Invalid block_id parameter: {}", block_id_param),
        }),
    }
}

/// Parses [BlockHash] from block_id url param
/// # Arguments
///
/// * `block_id` - Url parameter block_id.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// `block_id` supports different formats:
/// - `head` - return current block_hash from RpcCollectedStateRef
/// - `genesis` - return genesis from RpcCollectedStateRef
/// - `<level>` - return block which is on the level according to actual current_head branch
/// - `<block_hash>` - return block hash directly
/// - `<block>~<level>` - block can be: genesis/head/level/block_hash, e.g.: head~10 returns: the block which is 10 levels in the past from head)
/// - `<block>-<level>` - block can be: genesis/head/level/block_hash, e.g.: head-10 returns: the block which is 10 levels in the past from head)
/// - `<block>+<level>` - block can be: genesis/head/level/block_hash, e.g.: block_hash-10 returns: the block which is 10 levels after block_hash)
pub(crate) fn parse_block_hash(
    chain_id: &ChainId,
    block_id_param: &str,
    env: &RpcServiceEnvironment,
) -> Result<BlockHash, RpcServiceError> {
    // split header and optional offset (+, -, ~)
    let (block_param, offset_param) = {
        match block_id_param {
            bip if bip.contains('~') => split_block_id_param(block_id_param, '~', false)?,
            bip if bip.contains('-') => split_block_id_param(block_id_param, '-', false)?,
            bip if bip.contains('+') => split_block_id_param(block_id_param, '+', true)?,
            _ => (block_id_param, None),
        }
    };

    // closure for current head
    let current_head = || -> Result<(BlockHash, Level), RpcServiceError> {
        let state_read = env
            .state()
            .read()
            .map_err(|e| RpcServiceError::UnexpectedError {
                reason: format!("Lock state error: {}", e),
            })?;
        let current_head: &BlockHeaderWithHash = state_read.current_head().as_ref();
        Ok((current_head.hash.clone(), current_head.header.level()))
    };

    let genesis_block_hash = || -> Result<BlockHash, RpcServiceError> {
        if let Some(genesis) =
            ChainMetaStorage::new(env.persistent_storage()).get_genesis(chain_id)?
        {
            Ok(genesis.into())
        } else {
            return Err(RpcServiceError::InvalidParameters {
                reason: format!(
                    "No genesis found for chain_id: {}",
                    chain_id.to_base58_check()
                ),
            });
        }
    };

    let offset_from_current_head =
        |requested_level: Level| -> Result<(BlockHash, Option<u32>), RpcServiceError> {
            // we resolve offset as relative to current_head - requested_level
            let (current_head, current_head_level) = current_head()?;
            let offset_from_head = current_head_level - requested_level;
            let offset_from_head = if offset_from_head > current_head_level || offset_from_head < 0
            {
                return Err(RpcServiceError::NoDataFoundError { reason: format!("Offset({}) is too far from `head`, current_head_level: {}, requested_level: {}, block_id_param: {}", offset_from_head, current_head_level, requested_level, block_id_param) });
            } else {
                offset_from_head as u32
            };
            Ok((current_head, Some(offset_from_head)))
        };

    let (block_hash, offset) = match block_param {
        "head" => {
            let (current_head, _) = current_head()?;
            let offset_param = if let Some(offset) = offset_param {
                if offset < 0 {
                    return Err(RpcServiceError::InvalidParameters {
                        reason: format!("Offset for `head` parameter cannot be used with '+', block_id_param: {}",
                                        block_id_param)
                    });
                } else {
                    // ../head~<offset>/..
                    // ../head-<offset>/..
                    Some(offset as u32)
                }
            } else {
                // ../head/..
                None
            };
            (current_head, offset_param)
        }
        "genesis" => {
            if let Some(offset) = offset_param {
                if offset > 0 {
                    return Err(RpcServiceError::InvalidParameters { reason: format!("Offset for `genesis` parameter cannot be used with '~/-', block_id_param: {}", block_id_param) });
                } else {
                    // ../genesis+<offset>/.. - offset means level here

                    offset_from_current_head(offset.neg())?
                }
            } else {
                (genesis_block_hash()?, None)
            }
        }
        level_or_hash => {
            // try to parse level as number
            match level_or_hash.parse::<Level>() {
                // block level was passed as parameter to block_id_param
                Ok(requested_level) => {
                    // ../<level>/..
                    if offset_param.is_some() {
                        return Err(RpcServiceError::InvalidParameters {
                            reason: format!(
                                "Offset cannot be used with `level` parameter, block_id_param: {}",
                                block_id_param
                            ),
                        });
                    } else {
                        offset_from_current_head(requested_level)?
                    }
                }
                Err(_) => {
                    // block hash as base58 string was passed as parameter to block_id
                    match BlockHash::from_base58_check(level_or_hash) {
                        Ok(block_hash) => {
                            match offset_param {
                                Some(offset_value) => {
                                    if offset_value > 0 {
                                        // just find predecessor from block
                                        // ../<block_hash>~<offset>/..
                                        // ../<block_hash>-<offset>/..
                                        (block_hash, Some(offset_value as u32))
                                    } else if offset_value < 0 {
                                        // ../<block_hash>+<offset>/..

                                        // we can go from block_hash to head, but we need to calculate it from head.level backwards by predecessor
                                        let block_level = match BlockMetaStorage::new(env.persistent_storage()).get(&block_hash)? {
                                            Some(meta) => meta.level(),
                                            None => return Err(RpcServiceError::NoDataFoundError {
                                                reason: format!("BlockHeader was not found, block_id_param: {}, block_hash: {}",
                                                                block_id_param, block_hash.to_base58_check())
                                            })
                                        };
                                        let target_level = block_level - offset_value;
                                        offset_from_current_head(target_level)?
                                    } else {
                                        // ../<block_hash>~0/..
                                        // ../<block_hash>-0/..
                                        // very very ugly hack1 for genesis
                                        let genesis_block_hash = genesis_block_hash()?;
                                        if genesis_block_hash.eq(&block_hash) {
                                            (genesis_block_hash, None)
                                        } else {
                                            // very very ugly hack2 for genesis (genesis_hash is not the same as calculated from BlockHeader data)
                                            let real_genesis_block_hash = match BlockStorage::new(env.persistent_storage()).get(&genesis_block_hash)? {
                                                Some(block_header) => block_header.hash,
                                                None => return Err(RpcServiceError::UnexpectedError {
                                                    reason: format!("Genesis header was not found, genesis_block_hash: {}, block_id_param: {}",
                                                                    genesis_block_hash.to_base58_check(), block_id_param)
                                                })
                                            };
                                            if real_genesis_block_hash.eq(&block_hash) {
                                                (genesis_block_hash, None)
                                            } else {
                                                (block_hash, None)
                                            }
                                        }
                                    }
                                }
                                None => {
                                    // ../<block_hash>/..
                                    (block_hash, None)
                                }
                            }
                        }
                        Err(e) => {
                            // TODO: we should handle here other aliases: caboose, checkpoint, save_point...
                            return Err(RpcServiceError::InvalidParameters {
                                reason: format!(
                                    "Invalid/unhandled block_id_param: {}, reason: {}",
                                    block_id_param, e
                                ),
                            });
                        }
                    }
                }
            }
        }
    };

    // find requested header, if no offset we return header
    let block_hash = if let Some(offset) = offset {
        match BlockMetaStorage::new(env.persistent_storage())
            .find_block_at_distance(block_hash, offset)?
        {
            Some(block_hash) => block_hash,
            None => {
                return Err(RpcServiceError::NoDataFoundError {
                    reason: format!("Unknown block for block_id_param: {}", block_id_param),
                });
            }
        }
    } else {
        block_hash
    };

    Ok(block_hash)
}

pub(crate) async fn create_rpc_request(req: Request<Body>) -> Result<RpcRequest, anyhow::Error> {
    let context_path = req.uri().path_and_query().unwrap().as_str().to_string();
    let meth = RpcMethod::try_from(req.method().to_string().as_str()).unwrap(); // TODO: handle correctly
    let content_type = match req.headers().get(hyper::header::CONTENT_TYPE) {
        None => None,
        Some(hv) => Some(String::from_utf8(hv.as_bytes().into())?),
    };
    let accept = match req.headers().get(hyper::header::ACCEPT) {
        None => None,
        Some(hv) => Some(String::from_utf8(hv.as_bytes().into())?),
    };
    let body = hyper::body::to_bytes(req.into_body()).await?;
    let body = String::from_utf8(body.to_vec())?;

    Ok(RpcRequest {
        body,
        context_path: String::from(context_path.trim_end_matches('/')),
        meth,
        content_type,
        accept,
    })
}

/// Returns all prevalidator actors
pub(crate) fn get_prevalidators(
    env: &RpcServiceEnvironment,
) -> Result<Vec<Prevalidator>, RpcServiceError> {
    // TODO(vlad):
    let _ = env;
    Ok(vec![])
}

#[cfg(test)]
mod tests {
    use super::*;

    // NOTE: safe-guard in case `http` changes to decoding percent-encoding parts of the URI.
    // If that happens, update or remove this test and update the RPC-router in the OCaml
    // code so that it doesn't call `Uri.pct_decode` on the URI fragments. Code using the
    // query part of the URI may have to be updated too.
    #[test]
    fn test_pct_not_decoded() {
        let req = Request::builder()
            .uri("http://www.example.com/percent%20encoded?query=percent%20encoded")
            .body(())
            .unwrap();
        let path = req.uri().path_and_query().unwrap().as_str().to_string();
        let expected = "/percent%20encoded?query=percent%20encoded";
        assert_eq!(expected, &path);
    }
}
