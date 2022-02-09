// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, OperationListListHash};
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::operations_for_blocks::OperationsForBlocksMessage;
use tezos_messages::p2p::encoding::prelude::CurrentBranch;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BootstrapError {}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerIntervalState {
    pub peer: SocketAddr,
    pub downloaded: Vec<(Level, BlockHash, u8, OperationListListHash)>,
    pub current: Option<(Level, BlockHash)>,
}

impl PeerIntervalState {
    pub fn is_current_level_eq(&self, level: Level) -> bool {
        self.current
            .as_ref()
            .filter(|(current_level, _)| *current_level == level)
            .is_some()
    }

    pub fn is_current_hash_eq(&self, hash: &BlockHash) -> bool {
        self.current
            .as_ref()
            .filter(|(_, current_hash)| current_hash == hash)
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockWithDownloadedHeader {
    pub peer: SocketAddr,
    pub block_hash: BlockHash,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BootstrapBlockOperationGetState {
    pub block_level: Level,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,

    /// State of operations download from peer.
    ///
    /// We only request block operations from only 1 peer, but if that
    /// peer times out or disconnects, we need to request from other peer.
    pub peers: BTreeMap<SocketAddr, PeerBlockOperationsGetState>,
}

impl BootstrapBlockOperationGetState {
    pub fn is_success(&self) -> bool {
        self.peers.iter().any(|(_, p)| p.is_success())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerBlockOperationsGetState {
    Pending {
        time: u64,
        operations: Vec<Option<OperationsForBlocksMessage>>,
    },
    Success {
        time: u64,
        operations: Vec<OperationsForBlocksMessage>,
    },
}

impl PeerBlockOperationsGetState {
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Pending { operations, .. } => operations.iter().all(|v| v.is_some()),
            _ => false,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub fn is_validation_pass_pending(&self, validation_pass: u8) -> bool {
        match self {
            Self::Pending { operations, .. } => operations
                .get(validation_pass.max(0) as usize)
                .map(|v| v.is_none())
                .unwrap_or(false),
            _ => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BootstrapState {
    Idle {},

    Init {
        time: u64,
    },

    /// Wait until we have connected to minimum number of peers enough
    /// for bootstrapping.
    PeersConnectPending {
        time: u64,
    },
    PeersConnectSuccess {
        time: u64,
    },

    /// We have requested `GetCurrentBranch` from peers and we are waiting
    /// until we find a branch (block) on which majority of peers agree on.
    PeersMainBranchFindPending {
        time: u64,

        /// Current branches received from peers.
        peer_branches: BTreeMap<SocketAddr, CurrentBranch>,

        /// Block hashes and their supporting peers. Once we have found
        /// block hash on which majority of peers agree on, we start
        /// bootstrap from that block.
        block_supporters: BTreeMap<BlockHash, (Level, BTreeSet<SocketAddr>)>,
    },
    PeersMainBranchFindSuccess {
        time: u64,

        main_block: (Level, BlockHash),
        peer_branches: BTreeMap<SocketAddr, CurrentBranch>,
    },

    PeersBlockHeadersGetPending {
        time: u64,

        /// Level of the last(highest) block in the main_chain.
        main_chain_last_level: Level,

        /// Initialy only `main_block_hash` from prev state.
        ///
        /// When we find chains that are predecessors of the first block
        /// in this list, we prepend that chain to this list as we now
        /// know that chain is part of a branch that majority agreed on.
        main_chain: VecDeque<BlockWithDownloadedHeader>,

        /// TODO(zura): better name.
        ///
        /// Our current positions when downloading blocks from peers.
        /// Last blocks from this list that are direct predecessor of
        /// the main_chain (main_chain[0] pred = peer_branch_pointers.1)
        /// are moved to `main_chain`. Eventually this list will be empty.
        ///
        /// Once it's empty and we have reached our current head, when
        /// main_chain[0] == current_head, we will move to the next step.
        peer_intervals: Vec<PeerIntervalState>,
    },
    PeersBlockHeadersGetSuccess {
        time: u64,

        chain_last_level: Level,
        chain: VecDeque<BlockWithDownloadedHeader>,
    },

    PeersBlockOperationsGetPending {
        time: u64,

        last_level: Level,
        queue: VecDeque<BlockWithDownloadedHeader>,
        pending: BTreeMap<BlockHash, BootstrapBlockOperationGetState>,
    },
    PeersBlockOperationsGetSuccess {
        time: u64,
    },

    Error(BootstrapError),
}

impl BootstrapState {
    #[inline(always)]
    pub fn new() -> Self {
        Self::Idle {}
    }

    pub fn main_block(&self, peers_bootstrapped_min: usize) -> Option<(Level, BlockHash)> {
        match self {
            BootstrapState::PeersMainBranchFindPending {
                block_supporters, ..
            } => block_supporters
                .iter()
                .filter(|(_, (_, supporters))| supporters.len() >= peers_bootstrapped_min)
                .max_by(|(_, (level1, supporters1)), (_, (level2, supporters2))| {
                    supporters1
                        .len()
                        .cmp(&supporters2.len())
                        .then(level1.cmp(level2))
                })
                .map(|(block_hash, (level, _))| (*level, block_hash.clone())),
            _ => None,
        }
    }

    pub fn peer_intervals(&self) -> Option<&Vec<PeerIntervalState>> {
        match self {
            Self::PeersBlockHeadersGetPending { peer_intervals, .. } => Some(peer_intervals),
            _ => None,
        }
    }

    pub fn peer_intervals_mut(&mut self) -> Option<&mut Vec<PeerIntervalState>> {
        match self {
            Self::PeersBlockHeadersGetPending { peer_intervals, .. } => Some(peer_intervals),
            _ => None,
        }
    }

    pub fn peer_interval_pos(&self, peer: SocketAddr) -> Option<usize> {
        self.peer_intervals().and_then(|intervals| {
            intervals
                .iter()
                .rev()
                .position(|p| p.current.is_some() && p.peer.eq(&peer))
                .map(|i| intervals.len() - i - 1)
        })
    }

    pub fn peer_interval_by_level_pos(
        &self,
        peer: SocketAddr,
        block_level: Level,
    ) -> Option<usize> {
        let index = self.peer_interval_pos(peer)?;
        let is_level_eq = self
            .peer_intervals()?
            .get(index)?
            .is_current_level_eq(block_level);
        Some(index).filter(|_| is_level_eq)
    }

    pub fn peer_interval(&self, peer: SocketAddr) -> Option<&PeerIntervalState> {
        let index = self.peer_interval_pos(peer)?;
        self.peer_intervals()?.get(index)
    }

    pub fn peer_interval_mut(&mut self, peer: SocketAddr) -> Option<&mut PeerIntervalState> {
        let index = self.peer_interval_pos(peer)?;
        self.peer_intervals_mut()?.get_mut(index)
    }

    pub fn peer_interval_by_level(
        &self,
        peer: SocketAddr,
        block_level: Level,
    ) -> Option<&PeerIntervalState> {
        self.peer_interval(peer)
            .filter(|p| p.is_current_level_eq(block_level))
    }

    pub fn peer_interval_by_level_mut(
        &mut self,
        peer: SocketAddr,
        block_level: Level,
    ) -> Option<&mut PeerIntervalState> {
        self.peer_interval_mut(peer)
            .filter(|p| p.is_current_level_eq(block_level))
    }

    pub fn operations_get_queue_next(&self) -> Option<&BlockWithDownloadedHeader> {
        match self {
            Self::PeersBlockOperationsGetPending { queue, .. } => queue.front(),
            _ => None,
        }
    }

    pub fn operations_get_completed(
        &self,
        block_hash: &BlockHash,
    ) -> Option<&Vec<OperationsForBlocksMessage>> {
        match self {
            Self::PeersBlockOperationsGetPending { pending, .. } => pending
                .get(block_hash)
                .and_then(|b| b.peers.iter().find(|(_, p)| p.is_success()))
                .and_then(|(_, p)| match p {
                    PeerBlockOperationsGetState::Success { operations, .. } => Some(operations),
                    _ => None,
                }),
            _ => None,
        }
    }

    pub fn next_block_for_apply(&self) -> Option<&BlockHash> {
        match self {
            Self::PeersBlockOperationsGetPending { pending, .. } => pending
                .iter()
                .min_by_key(|(_, b)| b.block_level)
                .map(|(block_hash, _)| block_hash),
            _ => None,
        }
    }
}
