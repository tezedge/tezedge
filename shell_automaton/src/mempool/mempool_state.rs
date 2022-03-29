// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    net::SocketAddr,
};

use serde::{Deserialize, Serialize};

use crypto::hash::{
    BlockHash, BlockMetadataHash, CryptoboxPublicKeyHash, OperationHash,
    OperationMetadataListListHash,
};
use tezos_api::ffi::{Applied, Errored, PrevalidatorWrapper};
use tezos_messages::p2p::encoding::{
    block_header::{BlockHeader, Level},
    operation::Operation,
};

use crate::{
    prechecker::OperationDecodedContents, rights::Slot, service::rpc_service::RpcId, ActionWithMeta,
};

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct MempoolState {
    // TODO(vlad): instant
    pub running_since: Option<()>,
    //
    pub prevalidator: Option<PrevalidatorWrapper>,
    // performing rpc
    pub(super) injecting_rpc_ids: HashMap<OperationHash, RpcId>,
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
    pub(super) pending_operations: HashMap<OperationHash, Operation>,
    pub validated_operations: ValidatedOperations,
    // track ttl
    pub(super) level_to_operation: BTreeMap<i32, Vec<OperationHash>>,

    /// Last 120 (TTL) predecessor blocks.
    pub last_predecessor_blocks: HashMap<BlockHash, i32>,

    pub operation_stats: OperationsStats,

    pub old_operations_state: VecDeque<(Level, BTreeMap<OperationHash, MempoolOperation>)>,
    pub operations_state: BTreeMap<OperationHash, MempoolOperation>,

    /// Hash of the latest applied block
    pub latest_current_head: Option<BlockHash>,

    /// First current_head for the current level.
    pub first_current_head: bool,

    /// Timestamp of the first latest CurrentHead message
    pub first_current_head_time: u64,
}

impl MempoolState {
    /// Is endorsement for already applied block or not.
    pub fn is_old_consensus_operation(&self, chain_name: &str, operation: &Operation) -> bool {
        if !OperationKind::from_operation_content_raw(chain_name, operation.data().as_ref())
            .is_consensus_operation()
        {
            return false;
        }
        let level = match self.last_predecessor_blocks.get(operation.branch()) {
            Some(v) => *v,
            None => return false,
        };

        let current_head_level = self.local_head_state.as_ref().map(|b| b.header.level());
        current_head_level.map_or(false, |head_level| head_level < level + 1)
    }

    pub fn has_peer_seen_op(&self, peer: SocketAddr, op_hash: &OperationHash) -> bool {
        self.peer_state
            .get(&peer)
            .map_or(false, |p| p.seen_operations.contains(op_hash))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeadState {
    pub header: BlockHeader,
    pub hash: BlockHash,
    // prevalidator for the head is created
    pub prevalidator_ready: bool,

    pub metadata_hash: Option<BlockMetadataHash>,
    pub ops_metadata_hash: Option<OperationMetadataListListHash>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationStream {
    pub rpc_id: RpcId,
    pub applied: bool,
    pub refused: bool,
    pub branch_delayed: bool,
    pub branch_refused: bool,
    pub outdated: bool,
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
    pub outdated: Vec<Errored>,
}

#[derive(Default, Serialize, Deserialize, Debug, Clone)]
pub struct PeerState {
    // we received mempool from the peer and gonna send GetOperations
    pub(super) requesting_full_content: HashSet<OperationHash>,
    // those operations are known to the peer, should not rebroadcast
    pub(super) seen_operations: HashSet<OperationHash>,
}

pub type OperationsStats = BTreeMap<OperationHash, OperationStats>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct OperationStats {
    /// First time we saw this operation in the current head.
    pub kind: Option<OperationKind>,
    pub min_time: Option<u64>,
    pub first_block_timestamp: Option<u64>,
    pub validation_started: Option<u64>,
    /// (time_validation_finished, validation_result, prevalidation_duration)
    pub validation_result: Option<(u64, OperationValidationResult, Option<u64>, Option<u64>)>,
    pub validations: Vec<OperationValidationStats>,
    pub nodes: HashMap<CryptoboxPublicKeyHash, OperationNodeStats>,
    pub injected_timestamp: Option<u64>,
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
            injected_timestamp: None,
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
        preapply_started: Option<f64>,
        preapply_ended: Option<f64>,
        current_head_level: Option<i32>,
        result: OperationValidationResult,
    ) {
        // Convert seconds float to nanoseconds integer.
        let preapply_started =
            preapply_started.map(|preapply_started| (preapply_started * 1_000_000_000.0) as u64);
        let preapply_ended =
            preapply_ended.map(|preapply_ended| (preapply_ended * 1_000_000_000.0) as u64);

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
                v.preapply_started = preapply_started;
                v.preapply_ended = preapply_ended;
                v.result = Some(result);
            }
            None => {
                self.validations.push(OperationValidationStats {
                    started: None,
                    finished: Some(time),
                    preapply_started,
                    preapply_ended,
                    current_head_level,
                    result: Some(result),
                });
            }
        }
    }

    pub fn received_via_rpc(
        &mut self,
        chain_name: &str,
        self_pkh: &CryptoboxPublicKeyHash,
        stats: OperationNodeCurrentHeadStats,
        op_content: &[u8],
        injected_timestamp: &u64,
    ) {
        self.injected(injected_timestamp);
        self.set_kind_with(|| OperationKind::from_operation_content_raw(chain_name, op_content));
        self.min_time = Some(
            self.min_time
                .map_or(stats.time, |time| time.min(stats.time)),
        );
        if self.first_block_timestamp.is_none() && stats.block_timestamp >= 0 {
            self.first_block_timestamp = Some(stats.block_timestamp as u64);
        }

        let time = stats.time;
        if let Some(node_stats) = self.nodes.get_mut(self_pkh) {
            node_stats.received.push(stats);
            node_stats.content_received.push(time);
        } else {
            self.nodes.insert(
                self_pkh.clone(),
                OperationNodeStats {
                    received: vec![stats],
                    content_received: vec![time],
                    ..Default::default()
                },
            );
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
        if self.first_block_timestamp.is_none() && stats.block_timestamp >= 0 {
            self.first_block_timestamp = Some(stats.block_timestamp as u64);
        }

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.received.push(stats);
        } else {
            self.nodes.insert(
                node_pkh.clone(),
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
                node_pkh.clone(),
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
                node_pkh.clone(),
                OperationNodeStats {
                    content_requested: vec![time],
                    ..Default::default()
                },
            );
        }
    }

    pub fn content_received(
        &mut self,
        chain_name: &str,
        node_pkh: &CryptoboxPublicKeyHash,
        time: u64,
        op_content: &[u8],
    ) {
        self.set_kind_with(|| OperationKind::from_operation_content_raw(chain_name, op_content));
        self.min_time = Some(self.min_time.map_or(time, |t| t.min(time)));

        if let Some(node_stats) = self.nodes.get_mut(node_pkh) {
            node_stats.content_received.push(time);
        } else {
            self.nodes.insert(
                node_pkh.clone(),
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
                node_pkh.clone(),
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
                node_pkh.clone(),
                OperationNodeStats {
                    content_sent: vec![time],
                    ..Default::default()
                },
            );
        }
    }
    pub fn injected(&mut self, time: &u64) {
        self.injected_timestamp = Some(*time);
    }
}

impl Default for OperationStats {
    fn default() -> Self {
        Self::new()
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
    Outdated,

    Prechecked,
    PrecheckRefused,
    Prevalidate,
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
    SeedNonceRevelation,
    DoubleEndorsementEvidence,
    DoubleBakingEvidence,
    ActivateAccount,
    Proposals,
    Ballot,
    DoublePreendorsementEvidence,
    FailingNoop,
    Preendorsement,
    Endorsement,
    Reveal,
    Transaction,
    Origination,
    Delegation,
    RegisterGlobalConstant,
    SetDepositsLimit,

    /// Legacy! Used in Hangzhou, not in Ithaca.
    EndorsementWithSlot,

    Unknown,
}

impl OperationKind {
    pub fn from_operation_content_raw(chain_name: &str, bytes: &[u8]) -> Self {
        bytes
            .get(0)
            .map_or(Self::Unknown, |tag| Self::from_tag(chain_name, *tag))
    }

    pub fn from_tag(chain_name: &str, tag: u8) -> Self {
        if chain_name.contains("HANGZHOU") {
            Self::from_tag_hangzhou(tag)
        } else {
            Self::from_tag_ithaca(tag)
        }
    }

    pub fn from_tag_ithaca(tag: u8) -> Self {
        match tag {
            1 => Self::SeedNonceRevelation,
            2 => Self::DoubleEndorsementEvidence,
            3 => Self::DoubleBakingEvidence,
            4 => Self::ActivateAccount,
            5 => Self::Proposals,
            6 => Self::Ballot,
            7 => Self::DoublePreendorsementEvidence,
            17 => Self::FailingNoop,
            20 => Self::Preendorsement,
            21 => Self::Endorsement,
            107 => Self::Reveal,
            108 => Self::Transaction,
            109 => Self::Origination,
            110 => Self::Delegation,
            111 => Self::RegisterGlobalConstant,
            112 => Self::SetDepositsLimit,
            _ => Self::Unknown,
        }
    }

    pub fn from_tag_hangzhou(tag: u8) -> Self {
        match tag {
            0 => Self::Endorsement,
            1 => Self::SeedNonceRevelation,
            2 => Self::DoubleEndorsementEvidence,
            3 => Self::DoubleBakingEvidence,
            4 => Self::ActivateAccount,
            5 => Self::Proposals,
            6 => Self::Ballot,
            10 => Self::EndorsementWithSlot,
            17 => Self::FailingNoop,
            107 => Self::Reveal,
            108 => Self::Transaction,
            109 => Self::Origination,
            110 => Self::Delegation,
            111 => Self::RegisterGlobalConstant,
            _ => Self::Unknown,
        }
    }

    pub fn is_consensus_operation(&self) -> bool {
        matches!(
            self,
            Self::Preendorsement | Self::Endorsement | Self::EndorsementWithSlot
        )
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MempoolOperation {
    pub block_timestamp: i64,
    pub first_current_head: i64,
    pub state: OperationState,
    pub broadcast: bool,
    pub operation_decoded_contents: Option<OperationDecodedContents>,
    #[serde(flatten)]
    pub times: HashMap<String, i64>,
}

impl MempoolOperation {
    fn time_offset_from(action: &ActionWithMeta, base: i64) -> i64 {
        action.time_as_nanos() as i64 - base
    }

    fn time_offset(&self, action: &ActionWithMeta) -> i64 {
        Self::time_offset_from(action, self.first_current_head)
    }

    pub(super) fn received(
        mut block_timestamp: i64,
        first_current_head: u64,
        action: &ActionWithMeta,
    ) -> Self {
        let state = OperationState::ReceivedHash;
        block_timestamp *= 1_000_000_000;
        let first_current_head = first_current_head as i64;
        Self {
            block_timestamp,
            first_current_head,
            operation_decoded_contents: None,
            state,
            broadcast: false,
            times: HashMap::from([(
                state.time_name(),
                Self::time_offset_from(action, first_current_head),
            )]),
        }
    }

    pub(super) fn injected(
        mut block_timestamp: i64,
        first_current_head: u64,
        action: &ActionWithMeta,
    ) -> Self {
        let state = OperationState::ReceivedContents; // TODO use separate id
        block_timestamp *= 1_000_000_000;
        let first_current_head = first_current_head as i64;
        Self {
            block_timestamp,
            first_current_head,
            operation_decoded_contents: None,
            state,
            broadcast: false,
            times: HashMap::from([(
                state.time_name(),
                Self::time_offset_from(action, first_current_head),
            )]),
        }
    }

    pub(super) fn decoded(
        &self,
        operation_decoded_contents: OperationDecodedContents,
        action: &ActionWithMeta,
    ) -> Self {
        let state = OperationState::Decoded;
        let mut times = self.times.clone();
        times.insert(state.time_name(), self.time_offset(action));
        Self {
            times,
            state,
            operation_decoded_contents: Some(operation_decoded_contents),
            ..self.clone()
        }
    }

    pub(super) fn next_state(&self, state: OperationState, action: &ActionWithMeta) -> Self {
        let mut times = self.times.clone();
        times.insert(state.time_name(), self.time_offset(action));
        Self {
            times,
            state,
            ..self.clone()
        }
    }

    pub(super) fn broadcast(&self, action: &ActionWithMeta) -> Self {
        let mut times = self.times.clone();
        if !self.broadcast {
            times.insert("broadcast_time".to_string(), self.time_offset(action));
        }
        Self {
            times,
            broadcast: true,
            ..self.clone()
        }
    }

    pub(super) fn endorsement_slot(&self) -> Option<Slot> {
        self.operation_decoded_contents.as_ref()?.endorsement_slot()
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, strum_macros::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum OperationState {
    ReceivedHash,
    ReceivedContents,
    Decoded,
    Prechecked,
    Applied,

    PrecheckRefused,
    Refused,
    BranchRefused,
    BranchDelayed,
    Outdated,
}

impl OperationState {
    fn time_name(&self) -> String {
        self.to_string() + "_time"
    }
}
