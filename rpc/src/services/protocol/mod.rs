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

use failure::{bail, Fail};
use getset::Getters;
use itertools::Itertools;
use serde::Serialize;

use crypto::hash::{BlockHash, ChainId, HashType, ProtocolHash};
use storage::context::ContextApi;
use storage::{context_key, num_from_slice, BlockHeaderWithHash, BlockStorage, BlockStorageReader};
use tezos_api::ffi::{ProtocolRpcRequest, ProtocolRpcResponse, RpcRequest};
use tezos_messages::base::rpc_support::RpcJsonMap;
use tezos_messages::base::signature_public_key_hash::SignaturePublicKeyHash;
use tezos_messages::protocol::{
    proto_001 as proto_001_constants, proto_002 as proto_002_constants,
    proto_003 as proto_003_constants, proto_004 as proto_004_constants,
    proto_005 as proto_005_constants, proto_005_2 as proto_005_2_constants,
    proto_006 as proto_006_constants, proto_007 as proto_007_constants,
};

use crate::helpers::get_context_hash;
use crate::server::RpcServiceEnvironment;

mod proto_001;
mod proto_002;
mod proto_003;
mod proto_004;
mod proto_005_2;
mod proto_006;
mod proto_007;

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
    block_hash: BlockHash,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    max_priority: Option<&str>,
    has_all: bool,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, failure::Error> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(&block_hash, env)?;

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.hash_to_b58check(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH => {
            proto_001::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_002_constants::PROTOCOL_HASH => {
            proto_002::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_003_constants::PROTOCOL_HASH => {
            proto_003::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_004_constants::PROTOCOL_HASH => {
            proto_004::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_007_constants::PROTOCOL_HASH => {
            proto_007::rights_service::check_and_get_baking_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                env.tezedge_context(),
            )
        }
        _ => panic!(
            "Missing baking rights implemetation for protocol: {}, protocol is not yet supported!",
            hash
        ),
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
    block_hash: BlockHash,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    has_all: bool,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<RpcJsonMap>>, failure::Error> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(&block_hash, env)?;

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.hash_to_b58check(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH => {
            proto_001::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_002_constants::PROTOCOL_HASH => {
            proto_002::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_003_constants::PROTOCOL_HASH => {
            proto_003::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_004_constants::PROTOCOL_HASH => {
            proto_004::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
        }
        proto_007_constants::PROTOCOL_HASH => {
            proto_007::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                level,
                delegate,
                cycle,
                has_all,
                env.tezedge_context(),
            )
        }
        _ => panic!("Missing endorsing rights implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}

pub(crate) fn get_votes_listings(
    block_hash: BlockHash,
    env: &RpcServiceEnvironment,
) -> Result<Option<Vec<VoteListings>>, failure::Error> {
    let context_hash = get_context_hash(&block_hash, env)?;

    // filter out the listings data
    let listings_data = if let Some(val) = env
        .tezedge_context()
        .get_key_values_by_prefix(&context_hash, &context_key!("data/votes/listings"))?
    {
        val
    } else {
        bail!("No listings found in context")
    };

    // convert the raw context data to VoteListings
    let mut listings = Vec::with_capacity(listings_data.len());
    for (key, value) in listings_data.into_iter() {
        // get the address an the curve tag from the key (e.g. data/votes/listings/ed25519/2c/ca/28/ab/01/9ae2d8c26f4ce4924cad67a2dc6618)
        let keystr = key.join("/");
        let address = keystr.split('/').skip(4).take(6).join("");
        let curve = keystr.split('/').skip(3).take(1).join("");

        let address_decoded = SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?
            .to_string_representation();
        listings.push(VoteListings::new(
            address_decoded,
            num_from_slice!(value, 0, i32),
        ));
    }

    // sort the vector in reverse ordering (as in ocaml node)
    listings.sort();
    listings.reverse();
    Ok(Some(listings))
}

/// Struct for the delegates and they voting power (in rolls)
#[derive(Serialize, Debug, Clone, Getters, Eq, Ord, PartialEq, PartialOrd)]
pub struct VoteListings {
    /// Public key hash (address, e.g tz1...)
    #[get = "pub(crate)"]
    pkh: String,

    /// Number of rolls the pkh owns
    #[get = "pub(crate)"]
    rolls: i32,
}

impl VoteListings {
    /// Simple constructor to construct VoteListings
    pub fn new(pkh: String, rolls: i32) -> Self {
        Self { pkh, rolls }
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
) -> Result<Option<RpcJsonMap>, failure::Error> {
    let context_proto_params = get_context_protocol_params(block_hash, env)?;
    Ok(tezos_messages::protocol::get_constants_for_rpc(
        &context_proto_params.constants_data,
        context_proto_params.protocol_hash,
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
    let request =
        create_protocol_rpc_request(chain_param, chain_id, block_hash, rpc_request, &env)?;

    // TODO: TE-192 - refactor to protocol runner call
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
    pub protocol_hash: ProtocolHash,
    pub constants_data: Vec<u8>,
    pub block_header: BlockHeaderWithHash,
}

#[derive(Debug, Clone, Fail)]
pub enum ContextParamsError {
    #[fail(display = "Protocol not found in context for block: {}", _0)]
    NoProtocolForBlock(String),
    #[fail(display = "Protocol constants not found in context for block: {}", _0)]
    NoConstantsForBlock(String),
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
) -> Result<ContextProtocolParam, failure::Error> {
    // get block header
    let block_header = match BlockStorage::new(env.persistent_storage()).get(block_hash)? {
        Some(block) => block,
        None => bail!(
            "Block not found for block_hash: {}",
            HashType::BlockHash.hash_to_b58check(block_hash)
        ),
    };

    let protocol_hash: Vec<u8>;
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
            )
            .into());
        }

        if let Some(data) =
            context.get_key_from_history(&context_hash, &context_key!("data/v1/constants"))?
        {
            constants = data;
        } else {
            return Err(ContextParamsError::NoConstantsForBlock(
                HashType::BlockHash.hash_to_b58check(&block_hash),
            )
            .into());
        }
    };

    Ok(ContextProtocolParam {
        protocol_hash,
        constants_data: constants,
        block_header,
    })
}
