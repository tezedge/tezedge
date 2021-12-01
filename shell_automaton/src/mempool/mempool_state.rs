// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    convert::TryInto,
    net::SocketAddr,
};

use redux_rs::ActionId;
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, ChainId, HashBase58, OperationHash};
use tezos_api::ffi::{Applied, Errored, PrevalidatorWrapper};
use tezos_messages::p2p::encoding::{block_header::BlockHeader, operation::Operation};

use crate::service::rpc_service::RpcId;

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct MempoolState {
    // all blocks applied
    pub(super) applied_block: HashSet<BlockHash>,
    // do not create prevalidator for any applied block, create prevalidator:
    // * for block received as CurrentHead
    // * for block of injected operation
    pub prevalidator: Option<PrevalidatorWrapper>,
    //
    pub(super) requesting_prevalidator_for: Option<BlockHash>,
    // performing rpc
    pub(super) injecting_rpc_ids: HashMap<HashBase58<OperationHash>, RpcId>,
    // performed rpc
    pub(super) injected_rpc_ids: HashMap<HashBase58<OperationHash>, RpcId>,
    // the current head applied
    pub local_head_state: Option<(HeadState, BlockHash)>,
    // let's track what our peers know, and what we waiting from them
    pub(super) peer_state: HashMap<SocketAddr, PeerState>,
    // operations that passed basic checks, sent to protocol validator
    pub(super) pending_operations: HashMap<HashBase58<OperationHash>, Operation>,
    // operations that passed basic checks, are not sent because prevalidator is not ready
    pub(super) wait_prevalidator_operations: HashMap<HashBase58<OperationHash>, Operation>,
    pub validated_operations: ValidatedOperations,

    pub operations_state: BTreeMap<HashBase58<OperationHash>, OperationState>,

    pub current_head_timestamp: Option<ActionId>,
    pub new_current_head: Option<BlockHash>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ValidatedOperations {
    pub ops: HashMap<HashBase58<OperationHash>, Operation>,
    pub refused_ops: HashMap<HashBase58<OperationHash>, Operation>,
    // operations that passed all checks and classified
    // can be applied in the current context
    pub applied: Vec<Applied>,
    // cannot be included in the next head of the chain, but it could be included in a descendant
    pub branch_delayed: Vec<Errored>,
    // might be applied on a different branch if a reorganization happens
    pub branch_refused: Vec<Errored>,
    pub refused: Vec<Errored>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeadState {
    pub chain_id: ChainId,
    pub current_block: BlockHeader,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct PeerState {
    // we received mempool from the peer and gonna send GetOperations
    pub(super) requesting_full_content: HashSet<OperationHash>,
    // we sent GetOperations and pending full content of those operations
    pub(super) pending_full_content: HashSet<OperationHash>,
    // those operations are known to the peer, should not rebroadcast
    pub(super) seen_operations: HashSet<OperationHash>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "state", rename_all = "lowercase")]
pub enum OperationState {
    Received {
        receive_time: u64,
    },
    Prechecked {
        protocol_data: serde_json::Value,
        receive_time: u64,
        precheck_time: u64,
    },
    /* TODO
    Prevalidated {
        protocol_data: serde_json::Value,
        receive_time: u64,
        precheck_time: u64,
        prevalidate_time: u64,
    },
    Broadcast {
        protocol_data: serde_json::Value,
        receive_time: u64,
        precheck_time: u64,
        prevalidate_time: u64,
        broadcast_time: u64,
    }
    */
}

impl OperationState {
    pub(super) fn protocol_data(&self) -> Option<&serde_json::Value> {
        match self {
            OperationState::Prechecked { protocol_data, .. } => Some(protocol_data),
            _ => None,
        }
    }

    pub(super) fn endorsement_slot(&self) -> Option<&serde_json::Value> {
        let contents = self
            .protocol_data()?
            .as_object()?
            .get("contents")?
            .as_array()?;
        let contents_0 = if contents.len() == 1 {
            contents.get(0)?.as_object()?
        } else {
            return None;
        };
        match contents_0.get("kind")?.as_str()? {
            "endorsement_with_slot" => contents_0.get("slot"),
            _ => None,
        }
    }
}

impl MempoolState {
    pub(super) fn head_timestamp(&self) -> Option<u64> {
        let (HeadState { current_block, .. }, _) = self.local_head_state.as_ref()?;
        current_block.timestamp().try_into().map_or(None, Some)
    }

    pub(super) fn head_hash(&self) -> Option<&BlockHash> {
        self.new_current_head.as_ref()
    }
}
