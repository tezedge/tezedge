// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::SocketAddr,
};

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, CryptoboxPublicKeyHash, HashBase58, OperationHash};
use tezos_api::ffi::{Applied, Errored, PrevalidatorWrapper};
use tezos_messages::p2p::encoding::{block_header::BlockHeader, operation::Operation};

use crate::service::rpc_service::RpcId;

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct MempoolState {
    // TODO(vlad): instant
    pub running_since: Option<()>,
    // create prevalidator for any applied block, create prevalidator:
    pub prevalidator: Option<PrevalidatorWrapper>,
    // performing rpc
    pub(super) injecting_rpc_ids: HashMap<HashBase58<OperationHash>, RpcId>,
    // performed rpc
    pub(super) injected_rpc_ids: Vec<RpcId>,
    // operation streams requested by baker
    pub(super) operation_streams: Vec<OperationStream>,
    // the current head applied
    pub local_head_state: Option<HeadState>,
    pub branch_changed: bool,
    // let's track what our peers know, and what we waiting from them
    pub(super) peer_state: HashMap<SocketAddr, PeerState>,
    // we sent GetOperations and pending full content of those operations
    pub(super) pending_full_content: HashSet<OperationHash>,
    // operations that passed basic checks, sent to protocol validator
    pub(super) pending_operations: HashMap<HashBase58<OperationHash>, Operation>,
    // operations that passed basic checks, are not sent because prevalidator is not ready
    pub(super) wait_prevalidator_operations: Vec<Operation>,
    pub validated_operations: ValidatedOperations,
    // track ttl
    pub(super) level_to_operation: BTreeMap<i32, Vec<OperationHash>>,

    pub operation_stats: OperationsStats,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeadState {
    pub(super) header: BlockHeader,
    pub(super) hash: BlockHash,
    // operations included in the head already removed
    pub(super) ops_removed: bool,
    // prevalidator for the head is created
    pub(super) prevalidator_ready: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationStream {
    pub rpc_id: RpcId,
    pub applied: bool,
    pub refused: bool,
    pub branch_delayed: bool,
    pub branch_refused: bool,
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

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct PeerState {
    // we received mempool from the peer and gonna send GetOperations
    pub(super) requesting_full_content: HashSet<OperationHash>,
    // those operations are known to the peer, should not rebroadcast
    pub(super) seen_operations: HashSet<OperationHash>,
}

pub type OperationsStats = HashMap<HashBase58<OperationHash>, OperationStats>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationStats {
    pub validation_result: Option<(u64, OperationValidationResult)>,
    pub nodes: HashMap<HashBase58<CryptoboxPublicKeyHash>, OperationNodeStats>,
}

impl OperationStats {
    pub fn new() -> Self {
        Self {
            validation_result: None,
            nodes: HashMap::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum OperationValidationResult {
    Applied,
    Refused,
    BranchRefused,
    BranchDelayed,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationNodeStats {
    pub received: Vec<OperationNodeCurrentHeadStats>,
    pub sent: Vec<OperationNodeCurrentHeadStats>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationNodeCurrentHeadStats {
    pub time: u64,
    pub block_level: i32,
    pub block_timestamp: i64,
}
