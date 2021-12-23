// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    net::SocketAddr,
};

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, CryptoboxPublicKeyHash, HashBase58, OperationHash};
use tezos_api::ffi::{Applied, Errored};
use tezos_messages::p2p::encoding::{block_header::BlockHeader, operation::Operation};

use crate::service::rpc_service::RpcId;

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct MempoolState {
    // TODO(vlad): instant
    pub running_since: Option<()>,
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
    pub validated_operations: ValidatedOperations,
    // track ttl
    pub(super) level_to_operation: BTreeMap<i32, Vec<OperationHash>>,

    /// Last 120 (TTL) predecessor blocks.
    pub last_predecessor_blocks: HashMap<HashBase58<BlockHash>, i32>,

    pub operation_stats: OperationsStats,
}

impl MempoolState {
    /// Is endorsement for already applied block or not.
    pub fn is_old_endorsement(&self, operation: &Operation) -> bool {
        OperationKind::from_operation_content_raw(operation.data()).is_endorsement()
            && self
                .last_predecessor_blocks
                .contains_key(operation.branch())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeadState {
    pub(super) header: BlockHeader,
    pub(super) hash: BlockHash,
    // operations included in the head already removed
    pub(super) ops_removed: bool,
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
    // just validated
    pub(super) known_valid_to_send: Vec<OperationHash>,
}

pub type OperationsStats = HashMap<HashBase58<OperationHash>, OperationStats>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationStats {
    /// First time we saw this operation in the current head.
    pub kind: Option<OperationKind>,
    pub min_time: Option<u64>,
    pub first_block_timestamp: Option<u64>,
    pub validation_started: Option<u64>,
    /// (time_validation_finished, validation_result, prevalidation_duration)
    pub validation_result: Option<(u64, OperationValidationResult, u64, u64)>,
    pub validations: Vec<OperationValidationStats>,
    pub nodes: HashMap<HashBase58<CryptoboxPublicKeyHash>, OperationNodeStats>,
}

impl OperationStats {
    pub fn new() -> Self {
        Self {
            kind: None,
            min_time: None,
            first_block_timestamp: None,
            validation_started: None,
            validation_result: None,
            validations: vec![],
            nodes: HashMap::new(),
        }
    }

    /// Sets operation kind if not already set.
    pub fn set_kind_with<F: Fn() -> OperationKind>(&mut self, f: F) {
        if self.kind.is_none() {
            self.kind = Some(f());
        }
    }

    pub fn validation_started(&mut self, time: u64, current_head_level: Option<i32>) {
        if self
            .validation_result
            .filter(|x| x.1.is_applied())
            .is_none()
        {
            self.validation_started = Some(time);
            self.validation_result = None;
        }
        self.validations.push(OperationValidationStats {
            started: Some(time),
            finished: None,
            preapply_started: None,
            preapply_ended: None,
            current_head_level,
            result: None,
        });
    }

    pub fn validation_finished(
        &mut self,
        time: u64,
        preapply_started: f64,
        preapply_ended: f64,
        current_head_level: Option<i32>,
        result: OperationValidationResult,
    ) {
        // Convert seconds float to nanoseconds integer.
        let preapply_started = (preapply_started * 1_000_000_000.0) as u64;
        let preapply_ended = (preapply_ended * 1_000_000_000.0) as u64;

        if self
            .validation_result
            .filter(|x| x.1.is_applied())
            .is_none()
        {
            self.validation_result = Some((time, result, preapply_started, preapply_ended));
        }

        match self
            .validations
            .last_mut()
            .filter(|v| v.result.is_none())
            .filter(|v| v.current_head_level == current_head_level)
        {
            Some(v) => {
                v.finished = Some(time);
                v.preapply_started = Some(preapply_started);
                v.preapply_ended = Some(preapply_ended);
                v.result = Some(result);
            }
            None => {
                self.validations.push(OperationValidationStats {
                    started: None,
                    finished: Some(time),
                    preapply_started: Some(preapply_started),
                    preapply_ended: Some(preapply_ended),
                    current_head_level,
                    result: Some(result),
                });
            }
        }
    }

    pub fn received_in_current_head(
        &mut self,
        node_pkh: &CryptoboxPublicKeyHash,
        stats: OperationNodeCurrentHeadStats,
    ) {
        self.min_time = Some(
            self.min_time
                .map_or(stats.time, |time| time.min(stats.time)),
        );
        if self.first_block_timestamp.is_none() {
            if stats.block_timestamp >= 0 {
                self.first_block_timestamp = Some(stats.block_timestamp as u64);
            }
        }

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.received.push(stats);
        } else {
            self.nodes.insert(
                node_pkh.clone().into(),
                OperationNodeStats {
                    received: vec![stats],
                    ..Default::default()
                },
            );
        }
    }

    pub fn sent_in_current_head(
        &mut self,
        node_pkh: &CryptoboxPublicKeyHash,
        stats: OperationNodeCurrentHeadStats,
    ) {
        self.min_time = Some(
            self.min_time
                .map_or(stats.time, |time| time.min(stats.time)),
        );

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.sent.push(stats);
        } else {
            self.nodes.insert(
                node_pkh.clone().into(),
                OperationNodeStats {
                    sent: vec![stats],
                    ..Default::default()
                },
            );
        }
    }

    pub fn content_requested(&mut self, node_pkh: &CryptoboxPublicKeyHash, time: u64) {
        self.min_time = Some(self.min_time.map_or(time, |t| t.min(time)));

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.content_requested.push(time);
        } else {
            self.nodes.insert(
                node_pkh.clone().into(),
                OperationNodeStats {
                    content_requested: vec![time],
                    ..Default::default()
                },
            );
        }
    }

    pub fn content_received(
        &mut self,
        node_pkh: &CryptoboxPublicKeyHash,
        time: u64,
        op_content: &[u8],
    ) {
        self.set_kind_with(|| OperationKind::from_operation_content_raw(op_content));
        self.min_time = Some(self.min_time.map_or(time, |t| t.min(time)));

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.content_received.push(time);
        } else {
            self.nodes.insert(
                node_pkh.clone().into(),
                OperationNodeStats {
                    content_received: vec![time],
                    ..Default::default()
                },
            );
        }
    }

    pub fn content_requested_remote(&mut self, node_pkh: &CryptoboxPublicKeyHash, time: u64) {
        self.min_time = Some(self.min_time.map_or(time, |t| t.min(time)));

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.content_requested_remote.push(time);
        } else {
            self.nodes.insert(
                node_pkh.clone().into(),
                OperationNodeStats {
                    content_requested_remote: vec![time],
                    ..Default::default()
                },
            );
        }
    }

    pub fn content_sent(&mut self, node_pkh: &CryptoboxPublicKeyHash, time: u64) {
        self.min_time = Some(self.min_time.map_or(time, |t| t.min(time)));

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.content_sent.push(time);
        } else {
            self.nodes.insert(
                node_pkh.clone().into(),
                OperationNodeStats {
                    content_sent: vec![time],
                    ..Default::default()
                },
            );
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationValidationStats {
    pub started: Option<u64>,
    pub finished: Option<u64>,
    pub preapply_started: Option<u64>,
    pub preapply_ended: Option<u64>,
    pub current_head_level: Option<i32>,
    pub result: Option<OperationValidationResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum OperationValidationResult {
    Applied,
    Refused,
    BranchRefused,
    BranchDelayed,
}

impl OperationValidationResult {
    pub fn is_applied(&self) -> bool {
        matches!(self, Self::Applied)
    }
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct OperationNodeStats {
    pub received: Vec<OperationNodeCurrentHeadStats>,
    pub sent: Vec<OperationNodeCurrentHeadStats>,

    /// Timestamps when we have requested content of this operation from peer.
    pub content_requested: Vec<u64>,
    /// Timestamps when we have received content of this operation from peer.
    pub content_received: Vec<u64>,

    /// Timestamps when peer has requested content of this operation from us.
    pub content_requested_remote: Vec<u64>,
    /// Timestamps when we have sent content of this operation to peer.
    pub content_sent: Vec<u64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct OperationNodeCurrentHeadStats {
    pub time: u64,
    pub block_level: i32,
    pub block_timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub enum OperationKind {
    Endorsement,
    SeedNonceRevelation,
    DoubleEndorsement,
    DoubleBaking,
    Activation,
    Proposals,
    Ballot,
    EndorsementWithSlot,
    FailingNoop,
    Reveal,
    Transaction,
    Origination,
    Delegation,
    RegisterConstant,
    Unknown,
}

impl OperationKind {
    pub fn from_operation_content_raw(bytes: &[u8]) -> Self {
        bytes
            .get(0)
            .map_or(Self::Unknown, |tag| Self::from_tag(*tag))
    }

    pub fn from_tag(tag: u8) -> Self {
        match tag {
            0 => Self::Endorsement,
            1 => Self::SeedNonceRevelation,
            2 => Self::DoubleEndorsement,
            3 => Self::DoubleBaking,
            4 => Self::Activation,
            5 => Self::Proposals,
            6 => Self::Ballot,
            10 => Self::EndorsementWithSlot,
            17 => Self::FailingNoop,
            107 => Self::Reveal,
            108 => Self::Transaction,
            109 => Self::Origination,
            110 => Self::Delegation,
            111 => Self::RegisterConstant,
            _ => Self::Unknown,
        }
    }

    pub fn is_endorsement(&self) -> bool {
        matches!(self, Self::Endorsement | Self::EndorsementWithSlot)
    }
}
