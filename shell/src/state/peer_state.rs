// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crypto::hash::{BlockHash, OperationHash};
use networking::PeerId;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::limits;
use tezos_messages::p2p::encoding::prelude::MetadataMessage;

use crate::state::synchronization_state::UpdateIsBootstrapped;
use crate::state::StateError;

/// Limit to how many mempool operations to request in a batch
const MEMPOOL_OPERATIONS_BATCH_SIZE: usize = limits::MEMPOOL_MAX_OPERATIONS;

/// Holds information about a specific peer.
pub struct PeerState {
    /// PeerId identification (actor_ref + public key)
    pub(crate) peer_id: Arc<PeerId>,
    /// Has peer enabled mempool
    pub(crate) mempool_enabled: bool,
    /// Is bootstrapped flag
    pub(crate) is_bootstrapped: bool,

    /// Shareable data queues for scheduling of data download (blocks, operations)
    pub(crate) queues: Arc<DataQueues>,

    // TODO: TE-386 - global queue for requested operations
    /// Missing operations - we use this map for lazy/gracefull receiving
    pub(crate) missing_operations_for_blocks: HashMap<BlockHash, HashSet<i8>>,

    /// Level of the current head received from peer
    pub(crate) current_head_level: Option<i32>,
    /// Last time we received updated head from peer
    pub(crate) current_head_update_last: Instant,

    /// Last time we requested current head from the peer
    pub(crate) current_head_request_last: Instant,
    /// Last time we received current_head from the peer
    pub(crate) current_head_response_last: Instant,

    /// Last time we requested mempool operations from the peer
    pub(crate) mempool_operations_request_last: Option<Instant>,
    /// Last time we received mempool operations from the peer
    pub(crate) mempool_operations_response_last: Option<Instant>,

    /// Queued mempool operations. This map holds an operation hash and
    /// a tuple of type of a mempool operation with its time to live.
    pub(crate) queued_mempool_operations: HashSet<OperationHash>,

    /// Collected stats about p2p messages
    pub(crate) message_stats: MessageStats,
}

impl PeerState {
    pub fn new(
        peer_id: Arc<PeerId>,
        peer_metadata: &MetadataMessage,
        limits: DataQueuesLimits,
    ) -> Self {
        PeerState {
            peer_id,
            mempool_enabled: !peer_metadata.disable_mempool(),
            is_bootstrapped: false,
            queues: Arc::new(DataQueues::new(limits)),
            missing_operations_for_blocks: HashMap::default(),
            queued_mempool_operations: HashSet::default(),
            current_head_level: None,
            current_head_update_last: Instant::now(),
            current_head_request_last: Instant::now(),
            current_head_response_last: Instant::now(),
            mempool_operations_request_last: None,
            mempool_operations_response_last: None,
            message_stats: MessageStats::default(),
        }
    }

    pub fn available_mempool_operations_queue_capacity(&self) -> usize {
        let queued_count = self.queued_mempool_operations.len();
        if queued_count < MEMPOOL_OPERATIONS_BATCH_SIZE {
            MEMPOOL_OPERATIONS_BATCH_SIZE - queued_count
        } else {
            0
        }
    }

    /// Returns true, if was updated
    pub fn update_current_head_level(&mut self, new_level: Level) -> bool {
        // TODO: maybe fitness check?
        if self.current_head_level.is_none() || (self.current_head_level.unwrap() <= new_level) {
            self.current_head_level = Some(new_level);
            self.current_head_update_last = Instant::now();
            true
        } else {
            false
        }
    }

    pub fn clear(&mut self) {
        // self.queued_block_headers.clear();
        // self.queued_block_operations.clear();
        self.queued_mempool_operations.clear();
        self.missing_operations_for_blocks.clear();
    }

    pub fn mempool_operations_response_pending(&self, timeout: Duration) -> bool {
        let mempool_operations_request_last = match self.mempool_operations_request_last {
            Some(mempool_operations_request_last) => mempool_operations_request_last,
            None => {
                // no request, measn no pendings
                return false;
            }
        };

        match self.mempool_operations_response_last {
            Some(mempool_operations_response_last) => {
                // queued and we did receive last operation long time ago
                !self.queued_mempool_operations.is_empty()
                    && mempool_operations_response_last.elapsed() > timeout
            }
            None => {
                // queued and we did not receive any operations yet, but we requested long time ago
                !self.queued_mempool_operations.is_empty()
                    && mempool_operations_request_last.elapsed() > timeout
            }
        }
    }
}

/// Hold stats about peer received messages
pub struct MessageStats {
    unexpected_response_block: usize,
    unexpected_response_operations: usize,
}

impl MessageStats {
    pub fn increment_unexpected_response_block(&mut self) {
        self.unexpected_response_block += 1;
    }

    pub fn increment_unexpected_response_operations(&mut self) {
        self.unexpected_response_operations += 1;
    }
}

impl Default for MessageStats {
    fn default() -> Self {
        Self {
            unexpected_response_block: 0,
            unexpected_response_operations: 0,
        }
    }
}

pub type MissingOperations = HashSet<i8>;
pub type BlockHeaderQueueRef = Arc<Mutex<HashMap<Arc<BlockHash>, Instant>>>;
pub type BlockOperationsQueueRef =
    Arc<Mutex<HashMap<Arc<BlockHash>, (MissingOperations, Instant)>>>;

#[derive(Clone, Debug)]
pub struct DataQueues {
    pub(crate) limits: DataQueuesLimits,

    /// Queued blocks shared with peer_branch_bootstrapper
    pub(crate) queued_block_headers: BlockHeaderQueueRef,

    /// Queued block operations
    pub(crate) queued_block_operations: BlockOperationsQueueRef,
}

impl DataQueues {
    pub fn new(limits: DataQueuesLimits) -> Self {
        Self {
            limits,
            queued_block_headers: Arc::new(Mutex::new(HashMap::default())),
            queued_block_operations: Arc::new(Mutex::new(HashMap::default())),
        }
    }

    pub fn get_already_queued_block_headers_and_max_capacity(
        &self,
    ) -> Result<(HashSet<Arc<BlockHash>>, usize), StateError> {
        // lock, get queued and release
        let already_queued: HashSet<Arc<BlockHash>> =
            self.queued_block_headers.lock()?.keys().cloned().collect();

        let available =
            if already_queued.len() < self.limits.max_queued_block_headers_count as usize {
                (self.limits.max_queued_block_headers_count as usize) - already_queued.len()
            } else {
                0
            };

        Ok((already_queued, available))
    }

    pub fn get_already_queued_block_operations_and_max_capacity(
        &self,
    ) -> Result<(HashSet<Arc<BlockHash>>, usize), StateError> {
        // lock, get queued and release
        let already_queued: HashSet<Arc<BlockHash>> = self
            .queued_block_operations
            .lock()?
            .iter()
            .map(|(b, _)| b.clone())
            .collect();

        let available =
            if already_queued.len() < self.limits.max_queued_block_operations_count as usize {
                (self.limits.max_queued_block_operations_count as usize) - already_queued.len()
            } else {
                0
            };

        Ok((already_queued, available))
    }

    /// Returns tuple with block and duration how long is pending
    pub(crate) fn find_any_block_header_response_pending(
        &self,
        timeout: Duration,
    ) -> Result<Option<(Arc<BlockHash>, Duration)>, StateError> {
        Ok(self
            .queued_block_headers
            .lock()?
            .iter()
            .find(|(_, requested_time)| requested_time.elapsed().gt(&timeout))
            .map(|(block, requested_time)| (block.clone(), requested_time.elapsed())))
    }

    /// Returns tuple with block and duration how long is pending
    pub(crate) fn find_any_block_operations_response_pending(
        &self,
        timeout: Duration,
    ) -> Result<Option<(Arc<BlockHash>, Duration)>, StateError> {
        Ok(self
            .queued_block_operations
            .lock()?
            .iter()
            .find(|(_, (_, requested_time))| requested_time.elapsed().gt(&timeout))
            .map(|(block, (_, requested_time))| (block.clone(), requested_time.elapsed())))
    }
}

#[derive(Debug, Clone)]
pub struct DataQueuesLimits {
    /// Limit to how many blocks to request from peer
    /// Note: This limits speed of downloading chunked history
    pub(crate) max_queued_block_headers_count: u16,
    /// Limit to how many block operations to request from peer
    pub(crate) max_queued_block_operations_count: u16,
}

impl UpdateIsBootstrapped for PeerState {
    fn set_is_bootstrapped(&mut self, new_status: bool) {
        self.is_bootstrapped = new_status;
    }

    fn is_bootstrapped(&self) -> bool {
        self.is_bootstrapped
    }
}
