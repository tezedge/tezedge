// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{HashMap, HashSet},
    net::SocketAddr,
};

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, OperationHash};
use tezos_api::ffi::{Applied, Errored, PrevalidatorWrapper};
use tezos_messages::p2p::encoding::{block_header::BlockHeader, operation::Operation};

use crate::service::rpc_service::RpcId;

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct MempoolState {
    pub(super) is_bootstrapped: bool,
    // all blocks applied
    pub(super) applied_block: HashSet<BlockHash>,
    // do not create prevalidator for any applied block, create prevalidator:
    // * for block received as CurrentHead
    // * for block of injected operation
    pub prevalidator: Option<PrevalidatorWrapper>,
    // performing rpc
    pub(super) injecting_rpc_ids: HashMap<OperationHash, RpcId>,
    // performed rpc
    pub(super) injected_rpc_ids: Vec<RpcId>,
    // the current head applied
    pub local_head_state: Option<(BlockHeader, BlockHash)>,
    // let's track what our peers know, and what we waiting from them
    pub(super) peer_state: HashMap<SocketAddr, PeerState>,
    // operations that passed basic checks, sent to protocol validator
    pub(super) pending_operations: HashMap<OperationHash, Operation>,
    // operations that passed basic checks, are not sent because prevalidator is not ready
    pub(super) wait_prevalidator_operations: Vec<Operation>,
    pub validated_operations: ValidatedOperations,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct ValidatedOperations {
    pub ops: HashMap<OperationHash, Operation>,
    pub refused_ops: HashMap<OperationHash, Operation>,
    // operations that passed all checks and classified
    // can be applied in the current context
    pub applied: Vec<Applied>,
    // cannot be included in the next head of the chain, but it could be included in a descendant
    pub branch_delayed: Vec<Errored>,
    // might be applied on a different branch if a reorganization happens
    pub branch_refused: Vec<Errored>,
    pub refused: Vec<Errored>,
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
