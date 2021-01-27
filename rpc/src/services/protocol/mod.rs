// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module exposes protocol rpc services.
//!
//! Rule 1:
//!     - if service has different implementation and uses different structs for various protocols, it have to be placed and implemented in correct subpackage proto_XYZ
//!     - and here will be just redirector to correct subpackage by protocol_hash
//! Rule 2:
//!     - if service has the same implementation for various protocol, can be place directly here
//!     - if in new version of protocol is changed behavior, we have to splitted it here aslo by protocol_hash

use std::convert::TryInto;

use failure::{bail, Error, Fail};

use crypto::hash::{BlockHash, ChainId, HashType, ProtocolHash};
use storage::context::ContextApi;
use storage::{context_key, BlockHeaderWithHash, BlockStorage, BlockStorageReader};
use tezos_api::ffi::{
    HelpersPreapplyBlockRequest, ProtocolRpcRequest, ProtocolRpcResponse, RpcRequest,
};
use tezos_messages::base::rpc_support::RpcJsonMap;
use tezos_messages::protocol::{SupportedProtocol, UnsupportedProtocolError};

use crate::server::RpcServiceEnvironment;

mod proto_001;
mod proto_002;
mod proto_003;
mod proto_004;
mod proto_005_2;
mod proto_006;
mod proto_007;
mod proto_008;

#[derive(Debug, Fail)]
pub enum RightsError {
    #[fail(display = "Rights error, reason: {}", reason)]
    ServiceError { reason: Error },
    #[fail(display = "Unsupported protocol {}", protocol)]
    UnsupportedProtocolError { protocol: String },
}

impl From<ContextParamsError> for RightsError {
    fn from(error: ContextParamsError) -> Self {
        match error {
            ContextParamsError::UnsupportedProtocolError { protocol } => {
                RightsError::UnsupportedProtocolError { protocol }
            }
            _ => RightsError::ServiceError {
                reason: error.into(),
            },
        }
    }
}

impl From<failure::Error> for RightsError {
    fn from(error: failure::Error) -> Self {
        RightsError::ServiceError { reason: error }
    }
}

/// Return generated baking rights.
///
/// # Arguments
///
/// * `chain_id` - Url path parameter 'chain_id'.
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `max_priority` - Url query parameter 'max_priority'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate baking rights and then use Tezos PRNG to generate them.
pub(crate) fn check_and_get_baking_rights(
    block_hash: &BlockHash,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    max_priority: Option<&str>,
    has_all: bool,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, RightsError> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(block_hash, env)?;

    // split impl by protocol
    match context_proto_params.protocol_hash {
        SupportedProtocol::Proto001 => proto_001::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto002 => proto_002::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto003 => proto_003::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto004 => proto_004::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto005 => panic!("not yet implemented!"),
        SupportedProtocol::Proto005_2 => proto_005_2::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto006 => proto_006::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto007 => proto_007::rights_service::check_and_get_baking_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            max_priority,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto008 => {
            proto_008::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            ).map_err(RightsError::from)
        }
    }
}

/// Return generated endorsing rights.
///
/// # Arguments
///
/// * `chain_id` - Url path parameter 'chain_id'.
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `level` - Url query parameter 'level'.
/// * `delegate` - Url query parameter 'delegate'.
/// * `cycle` - Url query parameter 'cycle'.
/// * `has_all` - Url query parameter 'all'.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
///
/// Prepare all data to generate endorsing rights and then use Tezos PRNG to generate them.
pub(crate) fn check_and_get_endorsing_rights(
    block_hash: &BlockHash,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    has_all: bool,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, RightsError> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(block_hash, env)?;

    // split impl by protocol
    match context_proto_params.protocol_hash {
        SupportedProtocol::Proto001 => proto_001::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto002 => proto_002::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto003 => proto_003::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto004 => proto_004::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto005 => panic!("not yet implemented!"),
        SupportedProtocol::Proto005_2 => {
            proto_005_2::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
            .map_err(RightsError::from)
        }
        SupportedProtocol::Proto006 => proto_006::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto007 => proto_007::rights_service::check_and_get_endorsing_rights(
            context_proto_params,
            level,
            delegate,
            cycle,
            has_all,
            env.tezedge_context(),
        )
        .map_err(RightsError::from),
        SupportedProtocol::Proto008 => {
            proto_008::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            ).map_err(RightsError::from)
        }
    }
}

/// Get protocol context constants from context list
/// (just for RPC render use-case, do not use in processing or algorithms)
///
/// # Arguments
///
/// * `block_id` - Url path parameter 'block_id', it contains string "head", block level or block hash.
/// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
pub(crate) fn get_context_constants_just_for_rpc(
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Option<RpcJsonMap>, ContextParamsError> {
    let context_proto_params = get_context_protocol_params(block_hash, env)?;
    Ok(tezos_messages::protocol::get_constants_for_rpc(
        &context_proto_params.constants_data,
        &context_proto_params.protocol_hash,
    )?)
}

// TODO: TE-220, be more explicit about the kind of response from the RPC service
fn handle_rpc_response(
    response: &ProtocolRpcResponse,
    context_path: String,
) -> Result<serde_json::value::Value, failure::Error> {
    match response {
        ProtocolRpcResponse::RPCOk(body) => Ok(serde_json::from_str(&body)?),
        other => Err(failure::err_msg(format!(
            "Got non-OK response from protocol-RPC service '{}', reason: {:?}",
            context_path, other
        ))),
    }
}

pub(crate) fn call_protocol_rpc(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<serde_json::value::Value, failure::Error> {
    let context_path = rpc_request.context_path.clone();
    let request =
        create_protocol_rpc_request(chain_param, chain_id, block_hash, rpc_request, &env)?;

    // TODO: retry?
    let response = env
        .tezos_readonly_api()
        .pool
        .get()?
        .api
        .call_protocol_rpc(request)?;

    handle_rpc_response(&response, context_path)
}

pub(crate) fn preapply_operations(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<serde_json::value::Value, failure::Error> {
    let request =
        create_protocol_rpc_request(chain_param, chain_id, block_hash, rpc_request, &env)?;

    // TODO: retry?
    let response = env
        .tezos_readonly_api()
        .pool
        .get()?
        .api
        .helpers_preapply_operations(request)?;

    Ok(serde_json::from_str(&response.body)?)
}

pub(crate) fn preapply_block(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<serde_json::value::Value, failure::Error> {
    let block_storage = BlockStorage::new(env.persistent_storage());
    let (block_header, (predecessor_block_metadata_hash, predecessor_ops_metadata_hash)) =
        match block_storage.get_with_additional_data(&block_hash)? {
            Some((block_header, block_header_additional_data)) => {
                (block_header, block_header_additional_data.into())
            }
            None => bail!(
                "No block header found for hash: {}",
                HashType::BlockHash.hash_to_b58check(&block_hash)
            ),
        };

    // create request to ffi
    let request = HelpersPreapplyBlockRequest {
        protocol_rpc_request: ProtocolRpcRequest {
            chain_arg: chain_param.to_string(),
            block_header: block_header.header.as_ref().clone(),
            request: rpc_request,
            chain_id,
        },
        predecessor_block_metadata_hash,
        predecessor_ops_metadata_hash,
    };

    // TODO: retry?
    let response = env
        .tezos_readonly_api()
        .pool
        .get()?
        .api
        .helpers_preapply_block(request)?;

    Ok(serde_json::from_str(&response.body)?)
}

fn create_protocol_rpc_request(
    chain_param: &str,
    chain_id: ChainId,
    block_hash: BlockHash,
    rpc_request: RpcRequest,
    env: &RpcServiceEnvironment,
) -> Result<ProtocolRpcRequest, failure::Error> {
    let block_storage = BlockStorage::new(env.persistent_storage());
    let block_header = match block_storage.get(&block_hash)? {
        Some(header) => header.header.as_ref().clone(),
        None => bail!(
            "No block header found for hash: {}",
            HashType::BlockHash.hash_to_b58check(&block_hash)
        ),
    };

    // create request to ffi
    Ok(ProtocolRpcRequest {
        chain_arg: chain_param.to_string(),
        block_header,
        request: rpc_request,
        chain_id,
    })
}

pub(crate) struct ContextProtocolParam {
    pub protocol_hash: SupportedProtocol,
    pub constants_data: Vec<u8>,
    pub block_header: BlockHeaderWithHash,
}

#[derive(Debug, Fail)]
pub enum ContextParamsError {
    #[fail(display = "Protocol not found in context for block: {}", _0)]
    NoProtocolForBlock(String),
    #[fail(display = "Protocol constants not found in context for block: {}", _0)]
    NoConstantsForBlock(String),
    #[fail(display = "Storage error occurred, reason: {}", reason)]
    StorageError { reason: storage::StorageError },
    #[fail(display = "Context error occurred, reason: {}", reason)]
    ContextError {
        reason: storage::context::ContextError,
    },
    #[fail(display = "Context constants, reason: {}", reason)]
    ContextConstantsDecodeError {
        reason: tezos_messages::protocol::ContextConstantsDecodeError,
    },
    #[fail(display = "Unsupported protocol {}", protocol)]
    UnsupportedProtocolError { protocol: String },
}

impl From<storage::StorageError> for ContextParamsError {
    fn from(error: storage::StorageError) -> Self {
        ContextParamsError::StorageError { reason: error }
    }
}

impl From<storage::context::ContextError> for ContextParamsError {
    fn from(error: storage::context::ContextError) -> Self {
        ContextParamsError::ContextError { reason: error }
    }
}

impl From<UnsupportedProtocolError> for ContextParamsError {
    fn from(error: UnsupportedProtocolError) -> Self {
        ContextParamsError::UnsupportedProtocolError {
            protocol: error.protocol,
        }
    }
}

impl From<tezos_messages::protocol::ContextConstantsDecodeError> for ContextParamsError {
    fn from(error: tezos_messages::protocol::ContextConstantsDecodeError) -> Self {
        ContextParamsError::ContextConstantsDecodeError { reason: error }
    }
}

/// Get protocol and context constants as bytes from context list for desired block or level
///
/// # Arguments
///
/// * `block_hash` - [BlockHash]
/// * `opt_level` - Optionaly input block level from block_id if is already known to prevent double code execution.
/// * `list` - Context list handler.
/// * `persistent_storage` - Persistent storage handler.
/// * `state` - Current RPC collected state (head).
pub(crate) fn get_context_protocol_params(
    block_hash: &BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<ContextProtocolParam, ContextParamsError> {
    // get block header
    let block_header = match BlockStorage::new(env.persistent_storage()).get(block_hash)? {
        Some(block) => block,
        None => return Err(storage::StorageError::MissingKey.into()),
    };

    let protocol_hash: ProtocolHash;
    let constants: Vec<u8>;
    {
        let context = env.tezedge_context();
        let context_hash = block_header.header.context();

        if let Some(data) =
            context.get_key_from_history(&context_hash, &context_key!("protocol"))?
        {
            protocol_hash = data;
        } else {
            return Err(ContextParamsError::NoProtocolForBlock(
                HashType::BlockHash.hash_to_b58check(&block_hash),
            ));
        }

        if let Some(data) =
            context.get_key_from_history(&context_hash, &context_key!("data/v1/constants"))?
        {
            constants = data;
        } else {
            return Err(ContextParamsError::NoConstantsForBlock(
                HashType::BlockHash.hash_to_b58check(&block_hash),
            ));
        }
    };

    Ok(ContextProtocolParam {
        protocol_hash: protocol_hash.try_into()?,
        constants_data: constants,
        block_header,
    })
}
