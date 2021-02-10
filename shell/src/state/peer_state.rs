// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use riker::actors::*;

use crypto::hash::{BlockHash, OperationHash};
use networking::p2p::peer::SendMessage;
use networking::PeerId;
use storage::mempool_storage::MempoolOperationType;
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::prelude::{
    GetOperationsMessage, MetadataMessage, PeerMessageResponse,
};

use crate::peer_branch_bootstrapper::PeerBranchBootstrapperRef;
use crate::state::synchronization_state::UpdateIsBootstrapped;
use crate::state::MissingOperations;

// TODO: TE-386 - remove not needed
// /// Limit to how many blocks to request in a batch
// const BLOCK_HEADERS_BATCH_SIZE: usize = 10;
// /// Limit to how many block operations to request in a batch
// const BLOCK_OPERATIONS_BATCH_SIZE: usize = 15;
/// Limit to how many mempool operations to request in a batch
const MEMPOOL_OPERATIONS_BATCH_SIZE: usize = 20;
/// Mempool operation time to live
const MEMPOOL_OPERATION_TTL: Duration = Duration::from_secs(60);

/// Holds information about a specific peer.
pub struct PeerState {
    /// PeerId identification (actor_ref + public key)
    pub(crate) peer_id: Arc<PeerId>,
    /// Has peer enabled mempool
    pub(crate) mempool_enabled: bool,
    /// Is bootstrapped flag
    pub(crate) is_bootstrapped: bool,

    /// Actor for managing current branch bootstrap from peer
    pub(crate) peer_branch_bootstrapper: Option<PeerBranchBootstrapperRef>,

    // TODO: TE-386 - rename
    /// Queued blocks shared with peer_branch_bootstrapper
    pub(crate) queued_block_headers2: Arc<Mutex<HashSet<Arc<BlockHash>>>>,
    /// Queued block operations
    pub(crate) queued_block_operations2: Arc<Mutex<HashMap<BlockHash, MissingOperations>>>,

    // TODO: TE-386 - remove not needed
    // /// Missing blocks
    // pub(crate) missing_blocks: MissingBlockData<MissingBlock>,
    // TODO: TE-386 - global queue for requested operations
    /// Missing operations - we use this map for lazy/gracefull receiving
    pub(crate) missing_operations_for_blocks: HashMap<BlockHash, HashSet<i8>>,

    // TODO: TE-386 - remove not needed
    /// Queued blocks
    // pub(crate) queued_block_headers: HashMap<Arc<BlockHash>, MissingBlock>,
    // /// Queued block operations
    // pub(crate) queued_block_operations: HashMap<BlockHash, MissingOperations>,

    /// Level of the current head received from peer
    pub(crate) current_head_level: Option<i32>,
    /// Last time we received updated head from peer
    pub(crate) current_head_update_last: Instant,

    /// Last time we requested current head from the peer
    pub(crate) current_head_request_last: Instant,
    /// Last time we received current_head from the peer
    pub(crate) current_head_response_last: Instant,

    /// Last time we requested block from the peer
    pub(crate) block_request_last: Instant,
    /// Last time we received block from the peer
    pub(crate) block_response_last: Instant,

    /// Last time we requested block operations from the peer
    pub(crate) block_operations_request_last: Instant,
    /// Last time we received block operations from the peer
    pub(crate) block_operations_response_last: Instant,

    /// Last time we requested mempool operations from the peer
    pub(crate) mempool_operations_request_last: Instant,
    /// Last time we received mempool operations from the peer
    pub(crate) mempool_operations_response_last: Instant,

    /// Missing mempool operation hashes. Peer will be asked to provide operations for those hashes.
    /// After peer is asked for operation, this hash will be moved to `queued_mempool_operations`.
    pub(crate) missing_mempool_operations: Vec<(OperationHash, MempoolOperationType)>,
    /// Queued mempool operations. This map holds an operation hash and
    /// a tuple of type of a mempool operation with its time to live.
    pub(crate) queued_mempool_operations:
        HashMap<OperationHash, (MempoolOperationType, SystemTime)>,
}

impl PeerState {
    pub fn new(peer_id: Arc<PeerId>, peer_metadata: &MetadataMessage) -> Self {
        PeerState {
            peer_id,
            mempool_enabled: !peer_metadata.disable_mempool(),
            is_bootstrapped: false,
            peer_branch_bootstrapper: None,
            queued_block_headers2: Arc::new(Mutex::new(HashSet::default())),
            queued_block_operations2: Arc::new(Mutex::new(HashMap::default())),
            // missing_blocks: MissingBlockData::default(),
            missing_operations_for_blocks: HashMap::default(),
            // queued_block_headers: HashMap::new(),
            // queued_block_operations: HashMap::new(),
            missing_mempool_operations: Vec::new(),
            queued_mempool_operations: HashMap::default(),
            current_head_level: None,
            current_head_update_last: Instant::now(),
            current_head_request_last: Instant::now(),
            current_head_response_last: Instant::now(),
            block_request_last: Instant::now(),
            block_response_last: Instant::now(),
            block_operations_request_last: Instant::now(),
            block_operations_response_last: Instant::now(),
            mempool_operations_request_last: Instant::now(),
            mempool_operations_response_last: Instant::now(),
        }
    }

    // TODO: TE-386 - remove not needed
    // pub(crate) fn available_block_queue_capacity(&self) -> usize {
    //     let queued_count = self.queued_block_headers.len();
    //     if queued_count < BLOCK_HEADERS_BATCH_SIZE {
    //         BLOCK_HEADERS_BATCH_SIZE - queued_count
    //     } else {
    //         0
    //     }
    // }

    // TODO: TE-386 - remove not needed
    // fn available_block_operations_queue_capacity(&self) -> usize {
    //     let queued_count = self.queued_block_operations.len();
    //     if queued_count < BLOCK_OPERATIONS_BATCH_SIZE {
    //         BLOCK_OPERATIONS_BATCH_SIZE - queued_count
    //     } else {
    //         0
    //     }
    // }

    fn available_mempool_operations_queue_capacity(&self) -> usize {
        let queued_count = self.queued_mempool_operations.len();
        if queued_count < MEMPOOL_OPERATIONS_BATCH_SIZE {
            MEMPOOL_OPERATIONS_BATCH_SIZE - queued_count
        } else {
            0
        }
    }

    pub fn update_current_head(&mut self, block_header: &BlockHeaderWithHash) {
        // TODO: maybe fitness check?
        if self.current_head_level.is_none()
            || (block_header.header.level() >= self.current_head_level.unwrap())
        {
            self.current_head_level = Some(block_header.header.level());
            self.current_head_update_last = Instant::now();
        }
    }

    pub fn update_current_head_level(&mut self, new_level: Level) {
        // TODO: maybe fitness check?
        if self.current_head_level.is_none() || (self.current_head_level.unwrap() <= new_level) {
            self.current_head_level = Some(new_level);
            self.current_head_update_last = Instant::now();
        }
    }

    pub fn clear(&mut self) {
        self.missing_mempool_operations.clear();
        // self.queued_block_headers.clear();
        // self.queued_block_operations.clear();
        self.queued_mempool_operations.clear();
    }

    // TODO: TE-386 - remove not needed
    // pub fn schedule_missing_blocks<MD>(
    //     peers: &mut HashMap<ActorUri, PeerState>,
    //     mut get_missing_blocks: MD,
    // ) where
    //     MD: FnMut(&mut PeerState, usize, Option<i32>) -> Vec<MissingBlock>,
    // {
    //     peers
    //         .values_mut()
    //         .filter(|peer| peer.current_head_level.is_some())
    //         .filter(|peer| peer.available_block_queue_capacity() > 0)
    //         .sorted_by_key(|peer| peer.available_block_queue_capacity())
    //         .rev()
    //         .for_each(|peer| {
    //             let available_capacity = peer.available_block_queue_capacity();
    //             let peer_current_head_level = peer.current_head_level;
    //
    //             let mut missing_blocks =
    //                 get_missing_blocks(peer, available_capacity, peer_current_head_level);
    //
    //             if !missing_blocks.is_empty() {
    //                 let queued_blocks = missing_blocks
    //                     .drain(..)
    //                     .filter_map(|missing_block| {
    //                         let missing_block_hash = missing_block.block_hash.clone();
    //                         if peer
    //                             .queued_block_headers
    //                             .insert(missing_block_hash.clone(), missing_block)
    //                             .is_none()
    //                         {
    //                             // block was not already present in queue
    //                             Some(missing_block_hash.as_ref().clone())
    //                         } else {
    //                             // block was already in queue
    //                             None
    //                         }
    //                     })
    //                     .collect::<Vec<_>>();
    //
    //                 if !queued_blocks.is_empty() {
    //                     peer.block_request_last = Instant::now();
    //                     tell_peer(GetBlockHeadersMessage::new(queued_blocks).into(), peer);
    //                 }
    //             }
    //         });
    // }
    //
    // pub fn schedule_missing_operations<MD>(
    //     peers: &mut HashMap<ActorUri, PeerState>,
    //     mut get_missing_operations: MD,
    // ) where
    //     MD: FnMut(&mut PeerState, usize, Option<i32>) -> Vec<MissingOperations>,
    // {
    //     peers
    //         .values_mut()
    //         .filter(|peer| peer.current_head_level.is_some())
    //         .filter(|peer| peer.available_block_operations_queue_capacity() > 0)
    //         .sorted_by_key(|peer| peer.available_block_operations_queue_capacity())
    //         .rev()
    //         .for_each(|peer| {
    //             let available_capacity = peer.available_block_operations_queue_capacity();
    //             let peer_current_head_level = peer.current_head_level;
    //
    //             let missing_operations =
    //                 get_missing_operations(peer, available_capacity, peer_current_head_level);
    //
    //             if !missing_operations.is_empty() {
    //                 let queued_operations = missing_operations
    //                     .iter()
    //                     .map(|missing_operation| {
    //                         if peer
    //                             .queued_block_operations
    //                             .insert(
    //                                 missing_operation.block_hash.clone(),
    //                                 missing_operation.clone(),
    //                             )
    //                             .is_none()
    //                         {
    //                             // operations were not already present in queue
    //                             Some(missing_operation)
    //                         } else {
    //                             // operations were already in queue
    //                             None
    //                         }
    //                     })
    //                     .filter_map(|missing_operation| missing_operation)
    //                     .collect::<Vec<_>>();
    //
    //                 if !queued_operations.is_empty() {
    //                     peer.block_operations_request_last = Instant::now();
    //                     queued_operations.iter().for_each(|&missing_operation| {
    //                         tell_peer(
    //                             GetOperationsForBlocksMessage::new(missing_operation.into()).into(),
    //                             peer,
    //                         )
    //                     });
    //                 }
    //             }
    //         });
    // }

    pub fn schedule_missing_operations_for_mempool(peers: &mut HashMap<ActorUri, PeerState>) {
        peers
            .values_mut()
            .filter(|peer| !peer.missing_mempool_operations.is_empty())
            .filter(|peer| peer.available_mempool_operations_queue_capacity() > 0)
            .for_each(|peer| {
                let num_opts_to_get = cmp::min(
                    peer.missing_mempool_operations.len(),
                    peer.available_mempool_operations_queue_capacity(),
                );
                let ops_to_enqueue = peer
                    .missing_mempool_operations
                    .drain(0..num_opts_to_get)
                    .collect::<Vec<_>>();

                let ttl = SystemTime::now() + MEMPOOL_OPERATION_TTL;
                ops_to_enqueue
                    .iter()
                    .cloned()
                    .for_each(|(op_hash, op_type)| {
                        peer.queued_mempool_operations
                            .insert(op_hash, (op_type, ttl));
                    });

                let ops_to_get = ops_to_enqueue
                    .into_iter()
                    .map(|(op_hash, _)| op_hash)
                    .collect();

                peer.mempool_operations_request_last = Instant::now();
                tell_peer(GetOperationsMessage::new(ops_to_get).into(), peer);
            });
    }
}

impl UpdateIsBootstrapped for PeerState {
    fn set_is_bootstrapped(&mut self, new_status: bool) {
        self.is_bootstrapped = new_status;
    }

    fn is_bootstrapped(&self) -> bool {
        self.is_bootstrapped
    }
}

pub fn tell_peer(msg: Arc<PeerMessageResponse>, peer: &PeerState) {
    peer.peer_id.peer_ref.tell(SendMessage::new(msg), None);
}
