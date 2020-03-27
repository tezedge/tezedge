// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::HashType;
use storage::persistent::{ContextList, PersistentStorage};
use tezos_messages::protocol::{proto_001, proto_002, proto_003, proto_004, proto_005, proto_005_2, proto_006, RpcJsonMap};

use crate::helpers::get_context_protocol_params;
use crate::rpc_actor::RpcCollectedStateRef;

mod protocol;

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
        proto_001::PROTOCOL_HASH
        | proto_002::PROTOCOL_HASH
        | proto_003::PROTOCOL_HASH
        | proto_004::PROTOCOL_HASH
        | proto_005::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2::PROTOCOL_HASH => {
            protocol::proto_005_2::rights_service::check_and_get_baking_rights(
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
        proto_006::PROTOCOL_HASH => panic!("not yet implemented!"),
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
        proto_001::PROTOCOL_HASH
        | proto_002::PROTOCOL_HASH
        | proto_003::PROTOCOL_HASH
        | proto_004::PROTOCOL_HASH
        | proto_005::PROTOCOL_HASH => panic!("not yet implemented!"),
        proto_005_2::PROTOCOL_HASH => {
            protocol::proto_005_2::rights_service::check_and_get_endorsing_rights(
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
        proto_006::PROTOCOL_HASH => panic!("not yet implemented!"),
        _ => panic!("Missing endorsing rights implemetation for protocol: {}, protocol is not yet supported!", hash)
    }
}