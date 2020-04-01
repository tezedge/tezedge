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
use std::collections::HashMap;

use failure::bail;
use getset::Getters;
use itertools::Itertools;
use serde::Serialize;

use crypto::hash::HashType;
use storage::{num_from_slice, BlockStorage, BlockStorageReader};
use storage::persistent::{ContextList, ContextMap, PersistentStorage};
use storage::context::{TezedgeContext};
use storage::skip_list::Bucket;
use storage::context::{TezedgeContext, ContextIndex, ContextApi};
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
    UniversalValue,
    Ballot,
    BallotListElement,
    ToRpcJsonMap,
};


use crate::helpers::{get_context, get_context_protocol_params, get_level_by_block_id, extract_curve_and_bytes, BlockOperations, get_block_hash_by_block_id};
use crate::rpc_actor::RpcCollectedStateRef;
// use crate::server::service::Activity;

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
    list: ContextList,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<Vec<RpcJsonMap>>, failure::Error> {

    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        list.clone(),
        persistent_storage,
        state,
    )?;

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH
        | proto_002_constants::PROTOCOL_HASH
        | proto_003_constants::PROTOCOL_HASH
        | proto_004_constants::PROTOCOL_HASH
        | proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::rights_service::check_and_get_baking_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                max_priority,
                has_all,
                list,
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
                list,
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
    list: ContextList,
    persistent_storage: &PersistentStorage,
    state: &RpcCollectedStateRef) -> Result<Option<Vec<RpcJsonMap>>, failure::Error> {

    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        list.clone(),
        persistent_storage,
        state,
    )?;

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH
        | proto_002_constants::PROTOCOL_HASH
        | proto_003_constants::PROTOCOL_HASH
        | proto_004_constants::PROTOCOL_HASH
        | proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::rights_service::check_and_get_endorsing_rights(
                context_proto_params,
                chain_id,
                level,
                delegate,
                cycle,
                has_all,
                list,
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
                list,
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
            // let address = key.split("/").skip(4).take(6).join("");
            // let curve = key.split("/").skip(3).take(1).join("");

            let (curve, address) = if let Some(t) = extract_curve_and_bytes(&key)? {
                t
            } else {
                bail!("Cannot extract curve and address from key");
            };

            let address_decoded = SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?.to_string();
            listings.push(VoteListings::new(address_decoded, num_from_slice!(data, 0, i32)));
        }
    }

    // sort the vector in reverse ordering (as in ocaml node)
    listings.sort();
    listings.reverse();
    Ok(Some(listings))
}
//ed25519/a8/4d/01/3b/61b4c2cafe3fb89463329d7295a377
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

// pub(crate) fn get_votes_current_quorum

pub(crate) fn get_votes_current_quorum(
    chain_id: &str,
    block_id: &str,
    persistent_storage: &PersistentStorage,
    context_list: ContextList,
    state: &RpcCollectedStateRef) -> Result<Option<UniversalValue>, failure::Error> {

    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list);

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH
        | proto_002_constants::PROTOCOL_HASH
        | proto_003_constants::PROTOCOL_HASH
        | proto_004_constants::PROTOCOL_HASH
        | proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::votes_services::get_current_quorum(context_proto_params, chain_id, context)
        }
        proto_006_constants::PROTOCOL_HASH =>{
            proto_006::votes_services::get_current_quorum(context_proto_params, chain_id, context)
        }
        _ => panic!("Missing endorsing rights implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}

pub(crate) fn get_votes_current_proposal(_chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<UniversalValue>, failure::Error> {
    // get level by block_id
    let level: usize = if let Some(l) = get_level_by_block_id(block_id, persistent_storage, state)? {
        l
    } else {
        bail!("Level not found for block_id {}", block_id)
    };

    // prepare the context
    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list);
    let context_index = ContextIndex::new(Some(level), None);

    // get the current proposal data from the context db
    let current_proposal = if let Some(Bucket::Exists(data)) = context.get_key(&context_index, &vec!["data/votes/current_proposal".to_string()])? {
        // data
        // TODO: perform validation before using bytes_to_string
        HashType::ProtocolHash.bytes_to_string(&data)
    } else {
        // return None if the there is no current proposal (Ocaml behavior)
        return Ok(None)
    };
    
    Ok(Some(UniversalValue::String(current_proposal)))
}

pub(crate) fn get_votes_proposals(_chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<Vec<Vec<UniversalValue>>>, failure::Error> {
    // get level by block_id
    let level: usize = if let Some(l) = get_level_by_block_id(block_id, persistent_storage, state)? {
        l
    } else {
        bail!("Level not found for block_id {}", block_id)
    };

    // prepare context
    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list);
    let context_index = ContextIndex::new(Some(level), None);

    // key prefixes to look for in the context databse
    let proposals_key_prefix = vec!["data/votes/proposals/".to_string()];
    let listings_key_prefix = vec!["data/votes/listings/".to_string()];

    // get proposal data 
    let proposals_data = if let Some(data) = context.get_by_key_prefix(&context_index, &proposals_key_prefix)? {
        data
    } else {
        bail!("No proposals found");
    };

    // get listings data, we need to know each supporter's vote weight in rolls 
    let listings_data = if let Some(data) = context.get_by_key_prefix(&context_index, &listings_key_prefix)? {
        data
    } else {
        bail!("No proposals found");
    };

    let mut listings_map: HashMap<String, i32> = Default::default();

    // iterate over all the listings to get a map of delegates eith their weigth in rolls for the next step
    for (key, value) in listings_data.into_iter() {
        if let Bucket::Exists(data) = value {
            // get the address an the curve tag from the key (e.g. data/votes/listings/ed25519/2c/ca/28/ab/01/9ae2d8c26f4ce4924cad67a2dc6618)
            let (curve, address) = if let Some(t) = extract_curve_and_bytes(&key)? {
                t
            } else {
                bail!("Cannot extract curve and address from key");
            };

            let address_decoded = SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?.to_string();
            
            let roll_count = num_from_slice!(data, 0, i32);
            listings_map.insert(address_decoded, roll_count);
        }
    }
    
    let mut proposal_map: HashMap<String, i32> = Default::default();
    let ret: Vec<Vec<UniversalValue>>;

    // iterate over all the proposal data
    // proposals key example: data/votes/proposals/3e/5e/3a/60/6a/fab74a59ca09e333633e2770b6492c5e594455b71e9a2f0ea92afb/ed25519/a3/1e/81/ac/34/25310e3274a4698a793b2839dc0afa
    // components: - prefix: data/votes/proposals
    //             - protocol hash (proposal) in hex bytes: 3e/5e/3a/60/6a/fab74a59ca09e333633e2770b6492c5e594455b71e9a2f0ea92afb
    //             - curve and pkh in hex bytes of the supporter: ed25519/a3/1e/81/ac/34/25310e3274a4698a793b2839dc0afa
    for (key, value) in proposals_data.into_iter() {
        if let Bucket::Exists(_) = value  {
            let protocol_hash = HashType::ProtocolHash.bytes_to_string(&hex::decode(key.split("/").skip(3).take(6).join(""))?);

            let (curve, address) = if let Some(t) = extract_curve_and_bytes(&key)? {
                t
            } else {
                bail!("Cannot extract curve and address from key");
            };

            let address_decoded = SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?.to_string();
            
            // add the weight of the supporter to the sum for the concrete proposal
            if let Some(data) = listings_map.get(&address_decoded) {
                proposal_map.entry(protocol_hash)
                .and_modify(|val| *val = *val + data)
                .or_insert(data.clone());
            }
        }
    }

    // map to the return vector (to be compatible with ocaml)
    ret = proposal_map.into_iter()
        .map(|(k, v)| vec![UniversalValue::String(k), UniversalValue::Number(v)])
        .collect();

    Ok(Some(ret))
}

pub(crate) fn get_votes_ballot_list(_chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<Vec<RpcJsonMap>>, failure::Error> {
    // get level by block_id
    let level: usize = if let Some(l) = get_level_by_block_id(block_id, persistent_storage, state)? {
        l
    } else {
        bail!("Level not found for block_id {}", block_id)
    };

    // prepare context
    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list);
    let context_index = ContextIndex::new(Some(level), None);

    // prefix for the context search
    let ballot_key_prefix = vec!["data/votes/ballots/".to_string()];

    let ballot_data = if let Some(data) = context.get_by_key_prefix(&context_index, &ballot_key_prefix)? {
        data
    } else {
        bail!("No ballots found");
    };

    let mut temp_list: Vec<BallotListElement> = Default::default();

    // collect all the ballots into a temporary vector for sorting
    for (key, value) in ballot_data.into_iter() {
        if let Bucket::Exists(data) = value {
            if data.len() == 1 {
                // get the address an the curve tag from the key (e.g. data/votes/listings/ed25519/2c/ca/28/ab/01/9ae2d8c26f4ce4924cad67a2dc6618)
                let (curve, address) = if let Some(t) = extract_curve_and_bytes(&key)? {
                    t
                } else {
                    bail!("Cannot extract curve and address from key");
                };
                
                let address_decoded = SignaturePublicKeyHash::from_hex_hash_and_curve(&address, &curve)?;
                let ballot = match data[0] {
                    0 => Ballot::Yay,
                    1 => Ballot::Nay,
                    2 => Ballot::Pass,
                    _ => bail!("Wrong ballot")
                };
                temp_list.push(BallotListElement::new(address_decoded, ballot));
            } else {
                bail!("No ballot cast");
            }
        }
    }
    temp_list.sort();
    temp_list.reverse();

    // map the return RpcJsonMap to be compatible with ocaml
    let ballot_list = temp_list.iter()
        .map(|v| v.as_map())
        .collect();
    Ok(Some(ballot_list))
}

pub(crate) fn get_operations(_chain_id: &str, block_id: &str, persistent_storage: &PersistentStorage, state: &RpcCollectedStateRef) -> Result<Option<BlockOperations>, failure::Error>{
    let block_storage = BlockStorage::new(persistent_storage);
    let block_hash = get_block_hash_by_block_id(block_id, persistent_storage, state)?;
    let operations = block_storage.get_with_json_data(&block_hash)?.map(|(_, json_data)| json_data.operations_proto_metadata_json().clone());
    match operations {
        Some(op) => Ok(Some(serde_json::from_str(&op)?)),
        None => bail!("No operations found")
    }
}

pub(crate) fn get_delegate(chain_id: &str, block_id: &str, pkh: &str, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<RpcJsonMap>, failure::Error> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list);

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH
        | proto_002_constants::PROTOCOL_HASH
        | proto_003_constants::PROTOCOL_HASH
        | proto_004_constants::PROTOCOL_HASH
        | proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::delegate_service::get_delegate(context_proto_params, chain_id, pkh, context)
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::delegate_service::get_delegate(context_proto_params, chain_id, pkh, context)
        },
        _ => panic!("Missing delegates implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}

pub(crate) fn list_delegates(chain_id: &str, block_id: &str, activity: Activity, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<UniversalValue>, failure::Error> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list);

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH
        | proto_002_constants::PROTOCOL_HASH
        | proto_003_constants::PROTOCOL_HASH
        | proto_004_constants::PROTOCOL_HASH
        | proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::delegate_service::list_delegates(context_proto_params, chain_id, activity, context)
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::delegate_service::list_delegates(context_proto_params, chain_id, activity, context)
        },
        _ => panic!("Missing baking rights implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}

pub enum Activity {
    Active,
    Inactive,
    Both,
}

pub(crate) fn get_contract(chain_id: &str, block_id: &str, pkh: &str, persistent_storage: &PersistentStorage, context_list: ContextList, state: &RpcCollectedStateRef) -> Result<Option<RpcJsonMap>, failure::Error> {
    // get protocol and constants
    let context_proto_params = get_context_protocol_params(
        block_id,
        None,
        context_list.clone(),
        persistent_storage,
        state,
    )?;

    let context = TezedgeContext::new(BlockStorage::new(&persistent_storage), context_list);

    // split impl by protocol
    let hash: &str = &HashType::ProtocolHash.bytes_to_string(&context_proto_params.protocol_hash);
    match hash {
        proto_001_constants::PROTOCOL_HASH
        | proto_002_constants::PROTOCOL_HASH
        | proto_003_constants::PROTOCOL_HASH
        | proto_004_constants::PROTOCOL_HASH
        | proto_005_constants::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2_constants::PROTOCOL_HASH => {
            proto_005_2::contract_service::get_contract(context_proto_params, chain_id, pkh, context)
        }
        proto_006_constants::PROTOCOL_HASH => {
            proto_006::contract_service::get_contract(context_proto_params, chain_id, pkh, context)
        },
        _ => panic!("Missing baking rights implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}
