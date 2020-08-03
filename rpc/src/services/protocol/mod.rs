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

use failure::bail;
use getset::Getters;
use itertools::Itertools;
use serde::Serialize;

use crypto::hash::HashType;
use storage::{BlockStorage, BlockStorageReader, num_from_slice};
use storage::context::{TezedgeContext, ContextApi, ContextIndex};
use storage::persistent::{ContextList, ContextMap, PersistentStorage};
use storage::skip_list::Bucket;
use tezos_api::ffi::{FfiRpcService, JsonRpcRequest, ProtocolJsonRpcRequest};
use tezos_messages::base::signature_public_key_hash::SignaturePublicKeyHash;
use tezos_messages::protocol::{
    proto_001 as proto_001_constants,
    proto_002 as proto_002_constants,
    proto_003 as proto_003_constants,
    proto_004 as proto_004_constants,
    proto_005 as proto_005_constants,
    proto_005_2 as proto_005_2_constants,
    proto_006 as proto_006_constants,
    RpcJsonMap,
};
use tezos_messages::p2p::binary_message::BinaryMessage;

use crate::helpers::{ContextProtocolParam, Level, LevelConstants, get_block_hash_by_block_id, get_context, get_context_protocol_params, get_level_by_block_id};
use crate::rpc_actor::RpcCollectedStateRef;
use crate::server::RpcServiceEnvironment;
use crate::services::base_services::{get_block_by_block_id, get_block_level_by_block_id};

mod proto_001;
mod proto_002;
mod proto_003;
mod proto_004;
mod proto_005_2;
mod proto_006;

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
    chain_id: &str,
    block_id: &str,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    max_priority: Option<&str>,
    has_all: bool,
    context_list: ContextList,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<Vec<RpcJsonMap>>, failure::Error> {

    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list.clone());

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH => {
            proto_001::rights_service::check_and_get_baking_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_002_constants::PROTOCOL_HASH => {
            proto_002::rights_service::check_and_get_baking_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_003_constants::PROTOCOL_HASH => {
            proto_003::rights_service::check_and_get_baking_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_004_constants::PROTOCOL_HASH => {
            proto_004::rights_service::check_and_get_baking_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::rights_service::check_and_get_baking_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::rights_service::check_and_get_baking_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                context,
                persistent_storage,
            )
        }
        _ => panic!("Missing baking rights implemetation for protocol: {}, protocol is not yet supported!", hash)
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
    chain_id: &str,
    block_id: &str,
    level: Option<&str>,
    delegate: Option<&str>,
    cycle: Option<&str>,
    has_all: bool,
    context_list: ContextList,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<Vec<RpcJsonMap>>, failure::Error> {

    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list.clone());

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH => {
            proto_001::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_002_constants::PROTOCOL_HASH => {
            proto_002::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_003_constants::PROTOCOL_HASH => {
            proto_003::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_004_constants::PROTOCOL_HASH => {
            proto_004::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                has_all,
                context,
                persistent_storage,
            )
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                has_all,
                context,
                persistent_storage,
            )
        }
        _ => panic!("Missing endorsing rights implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}

pub(crate) fn get_votes_listings(_chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<Vec<VoteListings>>, failure::Error> {
    let mut listings = Vec::<VoteListings>::new();

    // get block level first
    let block_level: i64 = match get_level_by_block_id(block_id, persistent_storage, state)? {
        Some(val) => val.try_into()?,
        None => bail!("Block level not found")
    };

    // get the whole context
    let ctxt = get_context(&block_level.to_string(), context_list)?;

    // filter out the listings data
    let listings_data: ContextMap = ctxt.unwrap().into_iter()
        .filter(|(k, _)| k.starts_with(&"data/votes/listings/"))
        .collect();

    // convert the raw context data to VoteListings
    for (key, value) in listings_data.into_iter() {
        if let Bucket::Exists(data) = value {
            // get the address an the curve tag from the key (e.g. data/votes/listings/ed25519/2c/ca/28/ab/01/9ae2d8c26f4ce4924cad67a2dc6618)
            let address = key.split("/").skip(4).take(6).join("");
            let curve = key.split("/").skip(3).take(1).join("");

            let address_decoded = SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?.to_string();
            listings.push(VoteListings::new(address_decoded, num_from_slice!(data, 0, i32)));
        }
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
        Self {
            pkh,
            rolls,
        }
    }
}

pub(crate) fn proto_get_contract_counter(
    _chain_id: &str,
    block_id: &str,
    pkh: &str,
    context_list: ContextList,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<String>, failure::Error> {

    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list.clone());

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH |
        proto_002_constants::PROTOCOL_HASH |
        proto_003_constants::PROTOCOL_HASH |
        proto_004_constants::PROTOCOL_HASH |
        proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::contract_service::get_contract_counter(
                context_proto_params,
                pkh,
                context)
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::contract_service::get_contract_counter(
                context_proto_params,
                pkh,
                context)
        }
        _ => panic!("Missing contract counter implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}

pub(crate) fn proto_get_contract_manager_key(
    _chain_id: &str,
    block_id: &str,
    pkh: &str,
    context_list: ContextList,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<String>, failure::Error> {

    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list.clone());

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH |
        proto_002_constants::PROTOCOL_HASH |
        proto_003_constants::PROTOCOL_HASH |
        proto_004_constants::PROTOCOL_HASH |
        proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::contract_service::get_contract_manager_key(
                context_proto_params,
                pkh,
                context)
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::contract_service::get_contract_manager_key(
                context_proto_params,
                pkh,
                context)
        }
        _ => panic!("Missing manager key implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}

pub(crate) fn run_operation(chain_param: &str, block_param: &str, json_request: JsonRpcRequest, env: &RpcServiceEnvironment) -> Result<serde_json::value::Value, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let state = env.state();

    let request = create_protocol_json_rpc_request(chain_param, block_param, json_request, FfiRpcService::HelpersRunOperation, persistent_storage, state)?;

    // TODO: retry?
    let response = env.tezos_readonly_api().pool.get()?.api.call_protocol_json_rpc(request)?;

    Ok(serde_json::from_str(&response.body)?)
}

pub(crate) fn current_level(chain_param: &str, block_param: &str, json_request: JsonRpcRequest, env: &RpcServiceEnvironment) -> Result<serde_json::value::Value, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let state = env.state();

    let request = create_protocol_json_rpc_request(chain_param, block_param, json_request, FfiRpcService::HelpersCurrentLevel, persistent_storage, state)?;

    // TODO: retry?
    let response = env.tezos_readonly_api().pool.get()?.api.call_protocol_json_rpc(request)?;

    Ok(serde_json::from_str(&response.body)?)
}

pub(crate) fn minimal_valid_time(chain_param: &str, block_param: &str, json_request: JsonRpcRequest, env: &RpcServiceEnvironment) -> Result<serde_json::value::Value, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let state = env.state();
    
    let request = create_protocol_json_rpc_request(chain_param, block_param, json_request, FfiRpcService::DelegatesMinimalValidTime, persistent_storage, state)?;

    // TODO: retry?
    let response = env.tezos_readonly_api().pool.get()?.api.call_protocol_json_rpc(request)?;

    Ok(serde_json::from_str(&response.body)?)
}

pub(crate) fn live_blocks(chain_param: &str, block_param: &str, json_request: JsonRpcRequest, env: &RpcServiceEnvironment) -> Result<serde_json::value::Value, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let state = env.state();
    
    let request = create_protocol_json_rpc_request(chain_param, block_param, json_request, FfiRpcService::LiveBlocks, persistent_storage, state)?;

    // TODO: retry?
    let response = env.tezos_readonly_api().pool.get()?.api.call_protocol_json_rpc(request)?;

    Ok(serde_json::from_str(&response.body)?)
}


pub(crate) fn preapply_operations(chain_param: &str, block_param: &str, json_request: JsonRpcRequest, env: &RpcServiceEnvironment) -> Result<serde_json::value::Value, failure::Error> {
    let persistent_storage = env.persistent_storage();
    let state = env.state();

    let request = create_protocol_json_rpc_request(chain_param, block_param, json_request, FfiRpcService::HelpersPreapplyOperations, persistent_storage, state)?;

    // TODO: retry?
    let response = env.tezos_readonly_api().pool.get()?.api.helpers_preapply_operations(request)?;

    Ok(serde_json::from_str(&response.body)?)
}

pub(crate) fn preapply_block(chain_param: &str, block_param: &str, json_request: JsonRpcRequest, env: &RpcServiceEnvironment) -> Result<serde_json::value::Value, failure::Error> {
    // get header

    let persistent_storage = env.persistent_storage();
    let state = env.state();

    // create request to ffi
    let request = create_protocol_json_rpc_request(chain_param, block_param, json_request, FfiRpcService::HelpersPreapplyBlock, persistent_storage, state)?;

    // TODO: TE-192 - refactor to protocol runner call
    let response = env.tezos_readonly_api().pool.get()?.api.helpers_preapply_block(request)?;

    Ok(serde_json::from_str(&response.body)?)
}

fn create_protocol_json_rpc_request(chain_param: &str, block_param: &str, json_request: JsonRpcRequest, service: FfiRpcService, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<ProtocolJsonRpcRequest, failure::Error> {
    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_param, persistent_storage, state)?;
    let block_header = block_storage.get(&block_hash)?;
    let block_header = match block_header {
        Some(header) => header.header.as_ref().clone(),
        None => bail!("No block header found for hash: {}", block_param)
    };
    let state = state.read().unwrap();
    let chain_id = state.chain_id().clone();

    // create request to ffi
    Ok(ProtocolJsonRpcRequest {
        chain_arg: chain_param.to_string(),
        block_header,
        ffi_service: service,
        request: json_request,
        chain_id,
    })
}

// /// Get information about current head shell header
// pub(crate) fn get_level_info(block_id: &str, offset: Option<&str>, env: &RpcServiceEnvironment) -> Result<Option<Level>, failure::Error> {
//     let context_list = env.persistent_storage().context_storage();
//     let persistent_storage = env.persistent_storage();
//     let state = env.state();

//     // get protocol and constants
//     let context_proto_params = get_context_protocol_params(
//         block_id,
//         None,
//         context_list.clone(),
//         persistent_storage,
//         state,
//     )?;

//     let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);

//     // level constants dependant on protocols
//     let level_constants = get_level_constants(hash, context_proto_params)?;

//     let offset: Option<i32> = offset.map(|val| val.parse().unwrap());

//     let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list.clone());
//     // let context_index = ContextIndex::new(Some(level.into()), None);
//     // let first_level = context.get_key(&context_index, &vec!["data/v1/first_level"]);

//     // get the offster from the querry argument
//     // let offset = if let Some(offset) = offset {
//     //     match offset.parse() {
//     //         Ok(offset) => offset,
//     //         Err(e) => bail!("Offset must be a number")
//     //     }
//     // } else {
//     //     0
//     // };

//     let level = get_block_level_by_block_id(block_id, 0, persistent_storage, state)?;
//     // let level_info = Level::new(level: i32, first_level: i32, offset: Option<i32>, constants: LevelConstants)

//     // get the level struct from the block metadata
//     let level: Level = if let Some(full_block) = get_block_by_block_id(block_id, persistent_storage, state)? {
//         full_block.into()
//     } else {
//         bail!("Block not found");
//     };



//     Ok(Some(level))
// }

// type ConstantsBytes = Vec<u8>;

// /// Get the constants needed (to calculate relative levels) from the context
// fn get_level_constants(protocol_hash: &str, context_proto_params: ContextProtocolParam) -> Result<LevelConstants, failure::Error> {
//     match protocol_hash {
//         proto_001_constants::PROTOCOL_HASH => {
//             let dynamic = tezos_messages::protocol::proto_001::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;

//             let blocks_per_cycle = if let Some(blocks_per_cycle) = dynamic.blocks_per_cycle() {
//                 blocks_per_cycle
//             } else {
//                 bail!("No constant blocks_per_cycle found in block")
//             };

//             let blocks_per_voting_period = if let Some(blocks_per_voting_period) = dynamic.blocks_per_voting_period() {
//                 blocks_per_voting_period
//             } else {
//                 bail!("No constant blocks_per_voting_period found in block")
//             };

//             let blocks_per_commitment = if let Some(blocks_per_commitment) = dynamic.blocks_per_commitment() {
//                 blocks_per_commitment
//             } else {
//                 bail!("No constant blocks_per_commitment found in block")
//             };

//             Ok(LevelConstants::new(
//                 blocks_per_cycle,
//                 blocks_per_voting_period,
//                 blocks_per_commitment,
//             ))
//         }
//         proto_002_constants::PROTOCOL_HASH => {
//             let dynamic = tezos_messages::protocol::proto_002::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;

//             let blocks_per_cycle = if let Some(blocks_per_cycle) = dynamic.blocks_per_cycle() {
//                 blocks_per_cycle
//             } else {
//                 bail!("No constant blocks_per_cycle found in block")
//             };

//             let blocks_per_voting_period = if let Some(blocks_per_voting_period) = dynamic.blocks_per_voting_period() {
//                 blocks_per_voting_period
//             } else {
//                 bail!("No constant blocks_per_voting_period found in block")
//             };

//             let blocks_per_commitment = if let Some(blocks_per_commitment) = dynamic.blocks_per_commitment() {
//                 blocks_per_commitment
//             } else {
//                 bail!("No constant blocks_per_commitment found in block")
//             };

//             Ok(LevelConstants::new(
//                 blocks_per_cycle,
//                 blocks_per_voting_period,
//                 blocks_per_commitment,
//             ))
//         }
//         proto_003_constants::PROTOCOL_HASH => {
//             let dynamic = tezos_messages::protocol::proto_003::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;

//             let blocks_per_cycle = if let Some(blocks_per_cycle) = dynamic.blocks_per_cycle() {
//                 blocks_per_cycle
//             } else {
//                 bail!("No constant blocks_per_cycle found in block")
//             };

//             let blocks_per_voting_period = if let Some(blocks_per_voting_period) = dynamic.blocks_per_voting_period() {
//                 blocks_per_voting_period
//             } else {
//                 bail!("No constant blocks_per_voting_period found in block")
//             };

//             let blocks_per_commitment = if let Some(blocks_per_commitment) = dynamic.blocks_per_commitment() {
//                 blocks_per_commitment
//             } else {
//                 bail!("No constant blocks_per_commitment found in block")
//             };

//             Ok(LevelConstants::new(
//                 blocks_per_cycle,
//                 blocks_per_voting_period,
//                 blocks_per_commitment,
//             ))
//         }
//         proto_004_constants::PROTOCOL_HASH => {
//             let dynamic = tezos_messages::protocol::proto_004::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;

//             let blocks_per_cycle = if let Some(blocks_per_cycle) = dynamic.blocks_per_cycle() {
//                 blocks_per_cycle
//             } else {
//                 bail!("No constant blocks_per_cycle found in block")
//             };

//             let blocks_per_voting_period = if let Some(blocks_per_voting_period) = dynamic.blocks_per_voting_period() {
//                 blocks_per_voting_period
//             } else {
//                 bail!("No constant blocks_per_voting_period found in block")
//             };

//             let blocks_per_commitment = if let Some(blocks_per_commitment) = dynamic.blocks_per_commitment() {
//                 blocks_per_commitment
//             } else {
//                 bail!("No constant blocks_per_commitment found in block")
//             };

//             Ok(LevelConstants::new(
//                 blocks_per_cycle,
//                 blocks_per_voting_period,
//                 blocks_per_commitment,
//             ))
//         }
//         proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
//         proto_005_2_constants::PROTOCOL_HASH => {
//             let dynamic = tezos_messages::protocol::proto_005_2::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;

//             Ok(LevelConstants::new(
//                 dynamic.blocks_per_cycle(),
//                 dynamic.blocks_per_voting_period(),
//                 dynamic.blocks_per_commitment(),
//             ))
//         }
//         proto_006_constants::PROTOCOL_HASH => {
//             let dynamic = tezos_messages::protocol::proto_006::constants::ParametricConstants::from_bytes(context_proto_params.constants_data)?;

//             Ok(LevelConstants::new(
//                 dynamic.blocks_per_cycle(),
//                 dynamic.blocks_per_voting_period(),
//                 dynamic.blocks_per_commitment(),
//             ))
//         }
//         _ => panic!("Cannot get constants for Level interpretation, missing {} protocol", protocol_hash)
//     }
// }