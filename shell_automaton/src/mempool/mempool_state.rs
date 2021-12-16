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
    /// First time we saw this operation in the current head.
    pub kind: Option<OperationKind>,
    pub min_time: Option<u64>,
    pub validation_started: Option<u64>,
    pub validation_result: Option<(u64, OperationValidationResult)>,
    pub nodes: HashMap<HashBase58<CryptoboxPublicKeyHash>, OperationNodeStats>,
}

impl OperationStats {
    pub fn new() -> Self {
        Self {
            kind: None,
            min_time: None,
            validation_started: None,
            validation_result: None,
            nodes: HashMap::new(),
        }
    }

    /// Sets operation kind if not already set.
    pub fn set_kind_with<F: Fn() -> OperationKind>(&mut self, f: F) {
        if self.kind.is_none() {
            self.kind = Some(f());
        }
    }

    pub fn validation_started(&mut self, time: u64) {
        self.validation_started = Some(time);
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
pub enum OperationValidationResult {
    Applied,
    Refused,
    BranchRefused,
    BranchDelayed,
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
    Reveal,
    Transaction,
    Origination,
    Delegation,
    RegisterConstant,
    Unknown,
}

impl OperationKind {
    fn get_tag_bytes(bytes: &[u8]) -> Option<&[u8]> {
        bytes
            .iter()
            .position(|b| b & 0x80 == 0)
            .map(|i| &bytes[0..(i + 1)])
    }

    pub fn from_operation_content_raw(bytes: &[u8]) -> Self {
        // var value = BigInteger.Zero;

        let tag_bytes = match Self::get_tag_bytes(bytes) {
            Some(v) => v,
            None => return Self::Unknown,
        };

        let mut tag: u16 = 0;

        for b in tag_bytes.iter().rev().cloned() {
            tag = match tag.checked_shl(7) {
                Some(v) => v,
                None => return Self::Unknown,
            };
            tag |= (b & 0x7F) as u16;
        }

        Self::from_tag(tag)
    }

    pub fn from_tag(tag: u16) -> Self {
        match tag {
            0 => Self::Endorsement,
            1 => Self::SeedNonceRevelation,
            2 => Self::DoubleEndorsement,
            3 => Self::DoubleBaking,
            4 => Self::Activation,
            5 => Self::Proposals,
            6 => Self::Ballot,
            107 => Self::Reveal,
            108 => Self::Transaction,
            109 => Self::Origination,
            110 => Self::Delegation,
            111 => Self::RegisterConstant,
            _ => Self::Unknown,
        }
    }
}
