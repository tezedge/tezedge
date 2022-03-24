// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, OperationListListHash};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::operations_for_blocks::OperationsForBlocksMessage;

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;
#[cfg(feature = "fuzzing")]
use fuzzcheck::mutators::vector::VecMutator;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerIntervalState {
    pub peers: BTreeSet<SocketAddr>,
    pub downloaded: Vec<(Level, BlockHash, u8, OperationListListHash)>,
    pub current: PeerIntervalCurrentState,
}

impl PeerIntervalState {
    pub fn lowest_level(&self) -> Option<Level> {
        self.current
            .block_level()
            .or_else(|| self.downloaded.last().map(|(l, ..)| *l))
    }

    pub fn highest_level(&self) -> Option<Level> {
        let highest_downloaded = self.downloaded.first().map(|(l, ..)| *l);
        highest_downloaded.or_else(|| self.current.block_level())
    }

    pub fn lowest_and_highest_levels(&self) -> Option<(Level, Level)> {
        Some((self.lowest_level()?, self.highest_level()?))
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum PeerIntervalError {
    /// We can't accept interval as interval requires to change cemented
    /// block in the chain.
    CementedBlockReorg,

    /// Current interval's first block hash isn't equal to next interval's
    /// last block's predecessor hash.
    NextIntervalsPredecessorHashMismatch,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerIntervalCurrentState {
    Idle {
        time: u64,
        block_level: Level,
        block_hash: BlockHash,
    },
    Pending {
        time: u64,
        peer: SocketAddr,
        block_level: Level,
        block_hash: BlockHash,
    },
    Error {
        time: u64,
        peer: SocketAddr,
        block: BlockHeaderWithHash,
        error: PeerIntervalError,
    },
    Success {
        time: u64,
        peer: SocketAddr,
        block: BlockHeaderWithHash,
    },
    Finished {
        time: u64,
        peer: SocketAddr,
    },
    TimedOut {
        time: u64,
        peer: SocketAddr,
        block_level: Level,
        block_hash: BlockHash,
    },
    Disconnected {
        time: u64,
        peer: SocketAddr,
        block_level: Level,
        block_hash: BlockHash,
    },
}

impl PeerIntervalCurrentState {
    pub fn idle(time: u64, block_level: Level, block_hash: BlockHash) -> Self {
        Self::Idle {
            time,
            block_level,
            block_hash,
        }
    }

    pub fn peer(&self) -> Option<SocketAddr> {
        match self {
            Self::Pending { peer, .. }
            | Self::Success { peer, .. }
            | Self::Finished { peer, .. }
            | Self::TimedOut { peer, .. } => Some(*peer),
            Self::Disconnected { peer, .. } => Some(*peer),
            _ => None,
        }
    }

    pub fn block_level_with_hash(&self) -> Option<(Level, &BlockHash)> {
        match self {
            Self::Idle {
                block_level,
                block_hash,
                ..
            }
            | Self::Pending {
                block_level,
                block_hash,
                ..
            }
            | Self::TimedOut {
                block_level,
                block_hash,
                ..
            }
            | Self::Disconnected {
                block_level,
                block_hash,
                ..
            } => Some((*block_level, block_hash)),
            Self::Error { .. } => None,
            Self::Success { block, .. } => Some((block.header.level(), &block.hash)),
            Self::Finished { .. } => None,
        }
    }

    pub fn block_level(&self) -> Option<Level> {
        self.block_level_with_hash().map(|(level, _)| level)
    }

    pub fn block_hash(&self) -> Option<&BlockHash> {
        self.block_level_with_hash()
            .map(|(_, block_hash)| block_hash)
    }

    pub fn block(&self) -> Option<&BlockHeaderWithHash> {
        match self {
            Self::Success { block, .. } => Some(block),
            _ => None,
        }
    }

    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle { .. })
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. } | Self::TimedOut { .. })
    }

    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    pub fn is_finished(&self) -> bool {
        matches!(self, Self::Finished { .. })
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected { .. })
    }

    pub fn is_timed_out_or_disconnected(&self) -> bool {
        matches!(self, Self::TimedOut { .. } | Self::Disconnected { .. })
    }

    /// If we are in pending state and it is timed out.
    pub fn is_pending_timed_out(&self, timeout: u64, current_time: u64) -> bool {
        match self {
            Self::Pending { time, .. } => current_time - time >= timeout,
            _ => false,
        }
    }

    pub fn is_pending_block_level_eq(&self, other: Level) -> bool {
        match self {
            Self::Pending { block_level, .. } | Self::TimedOut { block_level, .. } => {
                *block_level == other
            }
            _ => false,
        }
    }

    pub fn is_pending_block_hash_eq(&self, other: &BlockHash) -> bool {
        match self {
            Self::Pending { block_hash, .. } | Self::TimedOut { block_hash, .. } => {
                block_hash == other
            }
            _ => false,
        }
    }

    pub fn is_pending_block_level_and_hash_eq(&self, level: Level, hash: &BlockHash) -> bool {
        match self {
            Self::Pending {
                block_level,
                block_hash,
                ..
            }
            | Self::TimedOut {
                block_level,
                block_hash,
                ..
            } => *block_level == level && block_hash == hash,
            _ => false,
        }
    }

    pub fn to_pending(&mut self, time: u64, peer: SocketAddr) {
        match self {
            Self::Idle {
                block_level,
                block_hash,
                ..
            }
            | Self::TimedOut {
                block_level,
                block_hash,
                ..
            }
            | Self::Disconnected {
                block_level,
                block_hash,
                ..
            } => {
                *self = Self::Pending {
                    time,
                    peer,
                    block_level: *block_level,
                    block_hash: block_hash.clone(),
                };
            }
            _ => {}
        }
    }

    pub fn to_success(&mut self, time: u64, block: BlockHeaderWithHash) {
        match self {
            Self::Pending { peer, .. } | Self::TimedOut { peer, .. } => {
                let peer = *peer;
                *self = Self::Success { time, peer, block };
            }
            _ => {}
        }
    }

    pub fn to_timed_out(&mut self, time: u64) {
        if let Self::Pending {
            peer,
            block_level,
            block_hash,
            ..
        } = self
        {
            *self = Self::TimedOut {
                time,
                peer: *peer,
                block_level: *block_level,
                block_hash: block_hash.clone(),
            };
        }
    }

    pub fn to_disconnected(&mut self, time: u64) {
        match self {
            Self::Pending {
                peer,
                block_level,
                block_hash,
                ..
            }
            | Self::TimedOut {
                peer,
                block_level,
                block_hash,
                ..
            } => {
                *self = Self::Disconnected {
                    time,
                    peer: *peer,
                    block_level: *block_level,
                    block_hash: block_hash.clone(),
                };
            }
            _ => {}
        }
    }

    pub fn to_finished(&mut self, time: u64, peer: SocketAddr) {
        *self = Self::Finished { time, peer };
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockWithDownloadedHeader {
    pub peer: Option<SocketAddr>,
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
    TimedOut {
        time: u64,
        operations: Vec<Option<OperationsForBlocksMessage>>,
    },
    Disconnected {
        time: u64,
    },
}

impl PeerBlockOperationsGetState {
    pub fn is_complete(&self) -> bool {
        match self {
            Self::Pending { operations, .. } | Self::TimedOut { operations, .. } => {
                operations.iter().all(|v| v.is_some())
            }
            _ => false,
        }
    }

    pub fn is_pending(&self) -> bool {
        matches!(self, Self::Pending { .. })
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(self, Self::Disconnected { .. })
    }

    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success { .. })
    }

    /// If we are in pending state and it is timed out.
    pub fn is_pending_timed_out(&self, timeout: u64, current_time: u64) -> bool {
        match self {
            Self::Pending { time, .. } => current_time - time >= timeout,
            _ => false,
        }
    }

    pub fn is_validation_pass_pending(&self, validation_pass: u8) -> bool {
        self.pending_operations()
            .and_then(|ops| ops.get(validation_pass.max(0) as usize))
            .map_or(false, |v| v.is_none())
    }

    pub fn to_timed_out(&mut self, time: u64) {
        let operations = match self {
            Self::Pending { operations, .. } => std::mem::take(operations),
            _ => return,
        };
        *self = Self::TimedOut { time, operations };
    }

    pub fn pending_operations(&self) -> Option<&Vec<Option<OperationsForBlocksMessage>>> {
        match self {
            Self::Pending { operations, .. } | Self::TimedOut { operations, .. } => {
                Some(operations)
            }
            _ => None,
        }
    }

    pub fn pending_operations_mut(
        &mut self,
    ) -> Option<&mut Vec<Option<OperationsForBlocksMessage>>> {
        match self {
            Self::Pending { operations, .. } | Self::TimedOut { operations, .. } => {
                Some(operations)
            }
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBranch {
    pub current_head: BlockHeaderWithHash,
    pub history: Vec<BlockHash>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum BootstrapError {
    CementedBlockReorg {
        current_head: BlockHeaderWithHash,
        block: BlockHeaderWithHash,
        /// Peers which agree on this reorg. We need to graylist them.
        #[cfg_attr(feature = "fuzzing", field_mutator(VecMutator<SocketAddr, SocketAddrMutator>))]
        peers: Vec<SocketAddr>,
    },
    BlockApplicationFailed,
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
        peer_branches: BTreeMap<SocketAddr, PeerBranch>,

        /// Block hashes and their supporting peers. Once we have found
        /// block hash on which majority of peers agree on, we start
        /// bootstrap from that block.
        block_supporters: BTreeMap<BlockHash, (BlockHeaderWithHash, BTreeSet<SocketAddr>)>,
    },
    PeersMainBranchFindSuccess {
        time: u64,

        main_block: BlockHeaderWithHash,
        peer_branches: BTreeMap<SocketAddr, PeerBranch>,
    },

    PeersBlockHeadersGetPending {
        time: u64,
        timeouts_last_check: Option<u64>,

        last_logged: u64,
        last_logged_downloaded_count: usize,

        /// Level of the last(highest) block in the main_chain.
        main_chain_last_level: Level,

        /// Hash of the last(highest) block in the main_chain.
        main_chain_last_hash: BlockHash,

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
        timeouts_last_check: Option<u64>,

        last_level: Level,
        queue: VecDeque<BlockWithDownloadedHeader>,
        pending: BTreeMap<BlockHash, BootstrapBlockOperationGetState>,
    },
    PeersBlockOperationsGetSuccess {
        time: u64,
    },

    Error {
        time: u64,
        error: BootstrapError,
    },

    /// We finished bootstrap pipeline.
    Finished {
        time: u64,
        error: Option<BootstrapError>,
    },
}

impl BootstrapState {
    #[inline(always)]
    pub fn new() -> Self {
        Self::Idle {}
    }

    pub fn timeouts_last_check(&self) -> Option<u64> {
        match self {
            Self::PeersBlockHeadersGetPending {
                time,
                timeouts_last_check,
                ..
            }
            | Self::PeersBlockOperationsGetPending {
                time,
                timeouts_last_check,
                ..
            } => timeouts_last_check.or(Some(*time)),
            _ => None,
        }
    }

    pub fn set_timeouts_last_check(&mut self, time: u64) {
        match self {
            Self::PeersBlockHeadersGetPending {
                timeouts_last_check,
                ..
            }
            | Self::PeersBlockOperationsGetPending {
                timeouts_last_check,
                ..
            } => {
                *timeouts_last_check = Some(time);
            }
            _ => {}
        }
    }

    pub fn main_block(&self, peers_bootstrapped_min: usize) -> Option<BlockHeaderWithHash> {
        match self {
            BootstrapState::PeersMainBranchFindPending {
                block_supporters, ..
            } => block_supporters
                .iter()
                .filter(|(_, (_, supporters))| supporters.len() >= peers_bootstrapped_min)
                .max_by(|(_, (block1, supporters1)), (_, (block2, supporters2))| {
                    supporters1
                        .len()
                        .cmp(&supporters2.len())
                        .then(block1.header.fitness().cmp(block2.header.fitness()))
                })
                .map(|(_, (block, _))| block.clone()),
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

    pub fn peer_next_interval(&self, peer: SocketAddr) -> Option<(usize, &PeerIntervalState)> {
        self.peer_intervals().and_then(|intervals| {
            intervals.iter().enumerate().find(|(_, p)| {
                (p.current.is_idle() || p.current.is_timed_out_or_disconnected())
                    && p.peers.contains(&peer)
            })
        })
    }

    pub fn peer_interval<F>(
        &self,
        peer: SocketAddr,
        predicate: F,
    ) -> Option<(usize, &PeerIntervalState)>
    where
        F: Fn(&PeerIntervalState) -> bool,
    {
        self.peer_intervals().and_then(|intervals| {
            intervals
                .iter()
                .enumerate()
                .rev()
                .filter(|(_, p)| p.current.peer().filter(|p| p == &peer).is_some())
                .find(|(_, p)| predicate(p))
        })
    }

    pub fn peer_next_interval_pos(&self, peer: SocketAddr) -> Option<usize> {
        self.peer_next_interval(peer).map(|(index, _)| index)
    }

    pub fn peer_interval_pos<F>(&self, peer: SocketAddr, predicate: F) -> Option<usize>
    where
        F: Fn(&PeerIntervalState) -> bool,
    {
        self.peer_interval(peer, predicate).map(|(index, _)| index)
    }

    pub fn peer_interval_mut<F>(
        &mut self,
        peer: SocketAddr,
        predicate: F,
    ) -> Option<(usize, &mut PeerIntervalState)>
    where
        F: Fn(&PeerIntervalState) -> bool,
    {
        let index = self.peer_interval_pos(peer, predicate)?;
        self.peer_intervals_mut()?
            .get_mut(index)
            .map(move |v| (index, v))
    }

    pub fn peer_next_interval_mut(
        &mut self,
        peer: SocketAddr,
    ) -> Option<(usize, &mut PeerIntervalState)> {
        let index = self.peer_next_interval_pos(peer)?;
        self.peer_intervals_mut()?
            .get_mut(index)
            .map(move |v| (index, v))
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

impl Default for BootstrapState {
    fn default() -> Self {
        Self::new()
    }
}
