// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! We need to fetch different data from p2p or send data to protocol validation
//! Main purpose of this module is to synchronize this request/responses per peers and handle queues management for peer
//!
//! We dont handle unique requests accross different peers, but if we want to, we just need to add here some synchronization.
//! Now we just handle unique requests per peer.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use riker::actors::*;
use slog::{warn, Logger};

use crypto::hash::{BlockHash, ChainId};
use networking::p2p::peer::SendMessage;
use networking::PeerId;
use storage::{BlockMetaStorage, BlockMetaStorageReader, OperationsMetaStorage};
use tezos_messages::p2p::encoding::limits;
use tezos_messages::p2p::encoding::prelude::{
    GetBlockHeadersMessage, GetOperationsForBlocksMessage, OperationsForBlock, PeerMessageResponse,
};

use crate::chain_feeder::{ApplyCompletedBlock, ChainFeederRef};
use crate::peer_branch_bootstrapper::PeerBranchBootstrapperRef;
use crate::state::peer_state::{DataQueues, MissingOperations, PeerState};
use crate::state::StateError;
use crate::utils::CondvarResult;
use crate::validation;

/// Shareable ref between threads
pub type DataRequesterRef = Arc<DataRequester>;

/// We controll frequecncy of apply block request from this actor, we dont want to exhausted chain_feeder
const APPLY_BLOCK_REQUEST_TIMEOUT_GLOBAL: Duration = Duration::from_secs(20);
const APPLY_BLOCK_REQUEST_TIMEOUT_PEER_PEER: Duration = Duration::from_secs(10);

/// Requester manages global request/response queues for data
/// and also manages local queues for every peer.
pub struct DataRequester {
    block_meta_storage: BlockMetaStorage,
    operations_meta_storage: OperationsMetaStorage,

    /// Chain feeder - actor, which is responsible to apply_block to context
    block_applier: ChainFeederRef,

    /// Shareable queue for limiting multiple duplicated requests for applying blocks
    queued_block_headers_for_apply: Arc<Mutex<HashMap<Arc<BlockHash>, Instant>>>,
}

impl DataRequester {
    pub fn new(
        block_meta_storage: BlockMetaStorage,
        operations_meta_storage: OperationsMetaStorage,
        block_applier: ChainFeederRef,
    ) -> Self {
        Self {
            block_meta_storage,
            operations_meta_storage,
            block_applier,
            queued_block_headers_for_apply: Arc::new(Mutex::new(HashMap::default())),
        }
    }

    /// Tries to schedule blocks downloading from peer
    ///
    /// Returns true if was scheduled and p2p message was sent
    pub fn fetch_block_headers(
        &self,
        blocks_to_download: Vec<Arc<BlockHash>>,
        peer: &PeerId,
        peer_queues: &DataQueues,
        log: &Logger,
    ) -> Result<bool, StateError> {
        // check if empty
        if blocks_to_download.is_empty() {
            return Ok(false);
        }

        // get queue locks
        let mut peer_queued_block_headers = peer_queues.queued_block_headers.lock()?;

        // calculate available capacity
        let available_capacity = if peer_queued_block_headers.len()
            < peer_queues.limits.max_queued_block_headers_count as usize
        {
            (peer_queues.limits.max_queued_block_headers_count as usize)
                - peer_queued_block_headers.len()
        } else {
            // full queue, we cannot schedule more
            return Ok(false);
        };

        // fillter non-queued
        let mut blocks_to_download = blocks_to_download
            .into_iter()
            .filter(|block_hash| !peer_queued_block_headers.contains(block_hash.as_ref()))
            .collect::<Vec<Arc<BlockHash>>>();

        // trim to max capacity
        let blocks_to_download = if available_capacity < blocks_to_download.len() {
            blocks_to_download.drain(0..available_capacity).collect()
        } else {
            blocks_to_download
        };

        // if empty finish
        if blocks_to_download.is_empty() {
            return Ok(false);
        }

        // add to queue
        let _ = peer_queued_block_headers.extend(blocks_to_download.clone());
        // release lock
        drop(peer_queued_block_headers);

        // send p2p msg - now we can fire msg to peer
        if limits::GET_BLOCK_HEADERS_MAX_LENGTH > 0 {
            blocks_to_download
                .chunks(limits::GET_BLOCK_HEADERS_MAX_LENGTH)
                .for_each(|blocks_to_download| {
                    tell_peer(
                        GetBlockHeadersMessage::new(
                            blocks_to_download
                                .iter()
                                .map(|b| b.as_ref().clone())
                                .collect::<Vec<BlockHash>>(),
                        )
                        .into(),
                        peer,
                    );
                });
        } else {
            tell_peer(
                GetBlockHeadersMessage::new(
                    blocks_to_download
                        .iter()
                        .map(|b| b.as_ref().clone())
                        .collect::<Vec<BlockHash>>(),
                )
                .into(),
                peer,
            );
        }

        // peer request stats
        match peer_queues.block_request_last.write() {
            Ok(mut request_last) => *request_last = Instant::now(),
            Err(e) => {
                warn!(log, "Failed to update block_request_last from peer"; "reason" => format!("{}", e),
                                "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
            }
        }

        Ok(true)
    }

    /// Tries to schedule blocks operations downloading from peer
    ///
    /// Returns true if was scheduled and p2p message was sent
    pub fn fetch_block_operations(
        &self,
        blocks_to_download: Vec<Arc<BlockHash>>,
        peer: &PeerId,
        peer_queues: &DataQueues,
        log: &Logger,
    ) -> Result<bool, StateError> {
        // check if empty
        if blocks_to_download.is_empty() {
            return Ok(false);
        }

        // get queue locks
        let mut peer_queued_block_headers = peer_queues.queued_block_operations.lock()?;

        // calculate available capacity
        let available_capacity = if peer_queued_block_headers.len()
            < peer_queues.limits.max_queued_block_operations_count as usize
        {
            (peer_queues.limits.max_queued_block_operations_count as usize)
                - peer_queued_block_headers.len()
        } else {
            // full queue, we cannot schedule more
            return Ok(false);
        };

        // fillter non-queued
        let mut blocks_to_download = blocks_to_download
            .into_iter()
            .filter(|block_hash| !peer_queued_block_headers.contains_key(block_hash.as_ref()))
            .collect::<Vec<Arc<BlockHash>>>();

        // trim to max capacity
        let blocks_to_download = if available_capacity < blocks_to_download.len() {
            blocks_to_download.drain(0..available_capacity).collect()
        } else {
            blocks_to_download
        };

        // collect missing validation_passes
        let blocks_to_download: Vec<(Arc<BlockHash>, MissingOperations)> = blocks_to_download
            .into_iter()
            .filter_map(|b| {
                if let Ok(Some(metadata)) = self.operations_meta_storage.get(&b) {
                    if let Some(missing_operations) = metadata.get_missing_validation_passes() {
                        if !missing_operations.is_empty() {
                            Some((b, missing_operations.iter().map(|vp| *vp as i8).collect()))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        // if empty just finish
        if blocks_to_download.is_empty() {
            return Ok(false);
        }

        // add to queue
        let _ = peer_queued_block_headers.extend(blocks_to_download.clone());
        // release lock
        drop(peer_queued_block_headers);

        // send p2p msg - now we can fire msg to peer
        if limits::GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH > 0 {
            let blocks_to_download: Vec<OperationsForBlock> = blocks_to_download
                .into_iter()
                .flat_map(|(block, missing_ops)| {
                    missing_ops
                        .into_iter()
                        .map(move |vp| OperationsForBlock::new(block.as_ref().clone(), vp))
                })
                .collect();

            blocks_to_download
                .chunks(limits::GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH)
                .for_each(|blocks_to_download| {
                    tell_peer(
                        GetOperationsForBlocksMessage::new(blocks_to_download.into()).into(),
                        peer,
                    );
                });
        } else {
            tell_peer(
                GetOperationsForBlocksMessage::new(
                    blocks_to_download
                        .into_iter()
                        .flat_map(|(block, missing_ops)| {
                            missing_ops
                                .into_iter()
                                .map(move |vp| OperationsForBlock::new(block.as_ref().clone(), vp))
                        })
                        .collect(),
                )
                .into(),
                peer,
            );
        }

        // peer request stats
        match peer_queues.block_operations_request_last.write() {
            Ok(mut request_last) => *request_last = Instant::now(),
            Err(e) => {
                warn!(log, "Failed to update block_operations_request_last from peer"; "reason" => format!("{}", e),
                                "peer_id" => peer.peer_id_marker.clone(), "peer_ip" => peer.peer_address.to_string(), "peer" => peer.peer_ref.name(), "peer_uri" => peer.peer_ref.uri().to_string());
            }
        }

        Ok(true)
    }

    /// Handle received block.
    ///
    /// Returns:
    ///     None - if was not scheduled for peer => unexpected block
    ///     Some(lock) - if was scheduled, lock is released, when comes out of the scope
    pub fn block_header_received(
        &self,
        block_hash: &BlockHash,
        peer: &mut PeerState,
        log: &Logger,
    ) -> Result<Option<RequestedBlockDataLock>, StateError> {
        // if was not scheduled, just return None
        if !peer
            .queues
            .queued_block_headers
            .lock()?
            .contains(block_hash)
        {
            warn!(log, "Received unexpected block header from peer"; "block_header_hash" => block_hash.to_base58_check());
            peer.message_stats.increment_unexpected_response_block();
            return Ok(None);
        }

        // peer response stats
        match peer.queues.block_response_last.write() {
            Ok(mut response_last) => *response_last = Instant::now(),
            Err(e) => {
                warn!(log, "Failed to update block_response_last from peer"; "reason" => format!("{}", e))
            }
        }

        // if contains, return data lock, when this lock will go out if the scope, then drop will be triggered, and queues will be emptied
        Ok(Some(RequestedBlockDataLock {
            block_hash: Arc::new(block_hash.clone()),
            queued_block_headers: peer.queues.queued_block_headers.clone(),
        }))
    }

    /// Handle received block operations which we requested.
    ///
    /// Returns:
    ///     None - if was not scheduled for peer => unexpected block/operations
    ///     Some(lock) - if was scheduled, lock is released, when comes out of the scope
    pub fn block_operations_received(
        &self,
        operations_for_block: &OperationsForBlock,
        peer: &mut PeerState,
        log: &Logger,
    ) -> Result<Option<RequestedOperationDataLock>, StateError> {
        let block_hash = operations_for_block.block_hash();
        let validation_pass = operations_for_block.validation_pass();

        // if was not scheduled, just return None
        match peer
            .queues
            .queued_block_operations
            .lock()?
            .get_mut(block_hash)
        {
            Some(missing_operations) => {
                if !missing_operations.contains(&validation_pass) {
                    warn!(log, "Received unexpected block header operation's validation pass from peer"; "block_header_hash" => block_hash.to_base58_check(), "validation_pass" => validation_pass);
                    peer.message_stats
                        .increment_unexpected_response_operations();
                    return Ok(None);
                }
            }
            None => {
                warn!(log, "Received unexpected block header operation from peer"; "block_header_hash" => block_hash.to_base58_check(), "validation_pass" => validation_pass);
                peer.message_stats
                    .increment_unexpected_response_operations();
                return Ok(None);
            }
        }

        // peer response stats
        match peer.queues.block_operations_response_last.write() {
            Ok(mut response_last) => *response_last = Instant::now(),
            Err(e) => {
                warn!(log, "Failed to update block_operations_response_last from peer"; "reason" => format!("{}", e))
            }
        }

        // if contains, return data lock, when this lock will go out if the scope, then drop will be triggered, and queues will be emptied
        Ok(Some(RequestedOperationDataLock {
            validation_pass,
            block_hash: Arc::new(block_hash.clone()),
            queued_block_operations: peer.queues.queued_block_operations.clone(),
        }))
    }

    /// Tries to schedule block for applying, if passed all checks
    ///
    /// Returns true, only if was block apply trigger (means, block is completed and can be applied and it is not already applied)
    pub fn try_schedule_apply_block(
        &self,
        block_to_apply: Arc<BlockHash>,
        chain_id: Arc<ChainId>,
        result_callback: Option<CondvarResult<(), failure::Error>>,
        bootstrapper: Option<PeerBranchBootstrapperRef>,
        peer_queued_block_headers_for_apply: Option<&mut HashMap<Arc<BlockHash>, Instant>>,
    ) -> Result<bool, StateError> {
        // 1. filter - lets check time-to-live request queue per peer
        if let Some(peer_queued_block_headers_for_apply) = peer_queued_block_headers_for_apply {
            if let Some(last_request) = peer_queued_block_headers_for_apply.get_mut(&block_to_apply)
            {
                if last_request.elapsed() > APPLY_BLOCK_REQUEST_TIMEOUT_PEER_PEER {
                    *last_request = Instant::now();
                } else {
                    return Ok(false);
                }
            } else {
                peer_queued_block_headers_for_apply.clear();
                peer_queued_block_headers_for_apply.insert(block_to_apply.clone(), Instant::now());
            }
        };

        // 2. filter - check if can be applied
        let block_metadata = match self.block_meta_storage.get(&block_to_apply)? {
            Some(block_metadata) => block_metadata,
            None => {
                return Err(StateError::ProcessingError {
                    reason: format!(
                        "No metadata found for block_hash: {}",
                        block_to_apply.to_base58_check()
                    ),
                });
            }
        };

        if !validation::can_apply_block(
            (&block_to_apply, &block_metadata),
            |bh| self.operations_meta_storage.is_complete(bh),
            |predecessor| self.block_meta_storage.is_applied(predecessor),
        )? {
            return Ok(false);
        }

        // 3. lets check global queue with lock
        {
            let mut queued_block_headers_for_apply = self.queued_block_headers_for_apply.lock()?;
            if let Some(last_request) = queued_block_headers_for_apply.get_mut(&block_to_apply) {
                if last_request.elapsed() > APPLY_BLOCK_REQUEST_TIMEOUT_GLOBAL {
                    // now we can schedule
                    *last_request = Instant::now();
                } else {
                    return Ok(false);
                }
            } else {
                // remove all older than APPLY_BLOCK_REQUEST_TIMEOUT_GLOBAL
                queued_block_headers_for_apply
                    .retain(|_, ttl| ttl.elapsed() <= APPLY_BLOCK_REQUEST_TIMEOUT_GLOBAL);

                // now we can schedule
                queued_block_headers_for_apply.insert(block_to_apply.clone(), Instant::now());
            }

            // explicit drop (not needed)
            drop(queued_block_headers_for_apply);
        }

        // 4. send to apply
        self.block_applier.tell(
            ApplyCompletedBlock::new(
                block_to_apply.as_ref().clone(),
                chain_id,
                result_callback,
                bootstrapper,
                Instant::now(),
            ),
            None,
        );

        Ok(true)
    }
}

/// Simple lock, when we want to remove data from queues,
/// but make sure that it was handled, and nobody will put the same data to queue, while we are handling them
///
/// When this lock goes out of the scope, then queues will be clear for block_hash
pub struct RequestedBlockDataLock {
    block_hash: Arc<BlockHash>,
    queued_block_headers: Arc<Mutex<HashSet<Arc<BlockHash>>>>,
}

impl Drop for RequestedBlockDataLock {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.queued_block_headers.lock() {
            queue.remove(&self.block_hash);
        }
    }
}

/// Simple lock, when we want to remove data from queues,
/// but make sure that it was handled, and nobody will put the same data to queue, while we are handling them
///
/// When this lock goes out of the scope, then queues will be clear for block_hash
pub struct RequestedOperationDataLock {
    validation_pass: i8,
    block_hash: Arc<BlockHash>,
    queued_block_operations: Arc<Mutex<HashMap<Arc<BlockHash>, MissingOperations>>>,
}

impl Drop for RequestedOperationDataLock {
    fn drop(&mut self) {
        if let Ok(mut queue) = self.queued_block_operations.lock() {
            if let Some(missing_operations) = queue.get_mut(&self.block_hash) {
                missing_operations.remove(&self.validation_pass);
                if missing_operations.is_empty() {
                    queue.remove(&self.block_hash);
                }
            }
        }
    }
}

fn tell_peer(msg: Arc<PeerMessageResponse>, peer: &PeerId) {
    peer.peer_ref.tell(SendMessage::new(msg), None);
}

#[cfg(test)]
mod tests {
    use std::collections::{HashMap, HashSet};
    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc::channel;
    use std::sync::{Arc, Mutex};
    use std::thread;

    use riker::actors::*;
    use serial_test::serial;
    use slog::Level;

    use crypto::hash::ChainId;
    use networking::p2p::network_channel::NetworkChannel;
    use storage::persistent::PersistentStorage;
    use storage::tests_common::TmpStorage;
    use storage::{
        block_meta_storage, operations_meta_storage, BlockMetaStorage, OperationsMetaStorage,
    };
    use tezos_messages::p2p::encoding::prelude::OperationsForBlock;

    use crate::chain_feeder::{ChainFeeder, ChainFeederRef};
    use crate::shell_channel::ShellChannel;
    use crate::state::data_requester::DataRequester;
    use crate::state::tests::block;
    use crate::state::tests::prerequisites::{
        create_logger, create_test_actor_system, create_test_tokio_runtime, test_peer,
    };
    use crate::state::StateError;

    macro_rules! assert_block_queue_contains {
        ($expected:expr, $queues:expr, $block:expr) => {{
            assert_eq!(
                $expected,
                $queues
                    .queued_block_headers
                    .lock()
                    .unwrap()
                    .contains($block)
            );
        }};
    }

    macro_rules! assert_operations_queue_contains {
        ($expected:expr, $queues:expr, $block:expr, $validation_passes:expr) => {{
            assert_eq!(
                $expected,
                $queues
                    .queued_block_operations
                    .lock()
                    .unwrap()
                    .contains_key($block)
            );

            match $queues.queued_block_operations.lock().unwrap().get($block) {
                Some(missing_operations) => assert_eq!(missing_operations, $validation_passes),
                None => {
                    if $expected {
                        panic!("test failed");
                    }
                }
            };
        }};
    }

    macro_rules! hash_set {
        ( $( $x:expr ),* ) => {
            {
                let mut temp_set = HashSet::new();
                $(
                    temp_set.insert($x);
                )*
                temp_set
            }
        };
    }

    #[test]
    #[serial]
    fn test_requester_fetch_and_receive_block() -> Result<(), failure::Error> {
        // prerequizities
        let log = create_logger(Level::Debug);
        let tokio_runtime = create_test_tokio_runtime();
        let actor_system = create_test_actor_system(log.clone());
        let network_channel =
            NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
        let storage = TmpStorage::create_to_out_dir("__test_requester_fetch_and_receive_block")?;
        let mut peer1 = test_peer(&actor_system, network_channel.clone(), &tokio_runtime, 7777);

        // requester instance
        let data_requester = DataRequester::new(
            BlockMetaStorage::new(storage.storage()),
            OperationsMetaStorage::new(storage.storage()),
            chain_feeder_mock(&actor_system, storage.storage().clone())?,
        );

        // try schedule nothiing
        assert!(matches!(
            data_requester.fetch_block_headers(vec![], &peer1.peer_id, &peer1.queues, &log),
            Ok(false)
        ));

        // try schedule block1
        let block1 = block(1);
        assert!(matches!(
            data_requester.fetch_block_headers(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(true)
        ));

        // scheduled just for the peer1
        assert_block_queue_contains!(true, peer1.queues, &block1);

        // try schedule block1 once more
        assert!(matches!(
            data_requester.fetch_block_headers(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(false)
        ));

        // try receive be peer1 - hold lock
        let scheduled = data_requester.block_header_received(&block1, &mut peer1, &log)?;
        assert!(scheduled.is_some());

        // block is still scheduled
        assert_block_queue_contains!(true, peer1.queues, &block1);

        // try schedule block1 once more while holding the lock (should not succeed, because block1 was not removed from queues, becuase we still hold the lock)
        assert!(matches!(
            data_requester.fetch_block_headers(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(false)
        ));

        // now drop/release lock
        drop(scheduled);

        // block is not scheduled now
        assert_block_queue_contains!(false, peer1.queues, &block1);

        // we can reschedule it once more now
        assert!(matches!(
            data_requester.fetch_block_headers(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(true)
        ));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_requester_fetch_and_receive_block_operations() -> Result<(), failure::Error> {
        // prerequizities
        let log = create_logger(Level::Debug);
        let tokio_runtime = create_test_tokio_runtime();
        let actor_system = create_test_actor_system(log.clone());
        let network_channel =
            NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
        let storage =
            TmpStorage::create_to_out_dir("__test_requester_fetch_and_receive_block_operations")?;
        let mut peer1 = test_peer(&actor_system, network_channel.clone(), &tokio_runtime, 7777);

        // requester instance
        let data_requester = DataRequester::new(
            BlockMetaStorage::new(storage.storage()),
            OperationsMetaStorage::new(storage.storage()),
            chain_feeder_mock(&actor_system, storage.storage().clone())?,
        );

        // prepare missing operations in db for block with 4 validation_pass
        let block1 = block(1);
        OperationsMetaStorage::new(storage.storage()).put(
            &block1,
            &operations_meta_storage::Meta::new(
                4,
                10,
                ChainId::from_base58_check("NetXgtSLGNJvNye")?,
            ),
        )?;

        // try schedule nothiing
        assert!(matches!(
            data_requester.fetch_block_operations(vec![], &peer1.peer_id, &peer1.queues, &log),
            Ok(false)
        ));

        // try schedule block1
        assert!(matches!(
            data_requester.fetch_block_operations(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(true)
        ));

        // scheduled just for the peer1
        assert_operations_queue_contains!(true, peer1.queues, &block1, &hash_set![0, 1, 2, 3]);

        // try schedule block1 once more
        assert!(matches!(
            data_requester.fetch_block_operations(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(false)
        ));

        // try receive be peer1 - hold lock - validation pass 0
        let scheduled = data_requester.block_operations_received(
            &OperationsForBlock::new(block1.as_ref().clone(), 0),
            &mut peer1,
            &log,
        )?;
        assert!(scheduled.is_some());

        // block is still scheduled
        assert_operations_queue_contains!(true, peer1.queues, &block1, &hash_set![0, 1, 2, 3]);

        // try schedule block1 once more while holding the lock (should not succeed, because block1 was not removed from queues, becuase we still hold the lock)
        assert!(matches!(
            data_requester.fetch_block_operations(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(false)
        ));

        // now drop/release lock
        drop(scheduled);

        // block is still scheduled but less validation passes
        assert_operations_queue_contains!(true, peer1.queues, &block1, &hash_set![1, 2, 3]);

        // we can reschedule it once more now
        assert!(matches!(
            data_requester.fetch_block_operations(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(false)
        ));

        // download all missing
        assert!(data_requester
            .block_operations_received(
                &OperationsForBlock::new(block1.as_ref().clone(), 1),
                &mut peer1,
                &log,
            )?
            .is_some());
        assert_operations_queue_contains!(true, peer1.queues, &block1, &hash_set![2, 3]);

        assert!(data_requester
            .block_operations_received(
                &OperationsForBlock::new(block1.as_ref().clone(), 3),
                &mut peer1,
                &log,
            )?
            .is_some());
        assert_operations_queue_contains!(true, peer1.queues, &block1, &hash_set![2]);

        assert!(data_requester
            .block_operations_received(
                &OperationsForBlock::new(block1.as_ref().clone(), 2),
                &mut peer1,
                &log,
            )?
            .is_some());
        assert_operations_queue_contains!(false, peer1.queues, &block1, &HashSet::default());

        // we can reschedule it once more now
        assert!(matches!(
            data_requester.fetch_block_operations(
                vec![block1.clone()],
                &peer1.peer_id,
                &peer1.queues,
                &log
            ),
            Ok(true)
        ));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_try_schedule_apply_block() -> Result<(), failure::Error> {
        // prerequizities
        let log = create_logger(Level::Debug);
        let actor_system = create_test_actor_system(log.clone());
        let storage = TmpStorage::create_to_out_dir("__test_try_schedule_apply_block")?;

        // requester instance
        let data_requester = DataRequester::new(
            BlockMetaStorage::new(storage.storage()),
            OperationsMetaStorage::new(storage.storage()),
            chain_feeder_mock(&actor_system, storage.storage().clone())?,
        );

        // prepare missing operations in db for block with 4 validation_pass
        let block1 = block(1);
        let chain_id = Arc::new(ChainId::from_base58_check("NetXgtSLGNJvNye")?);

        // try schedule - no metadata
        assert!(matches!(
            data_requester.try_schedule_apply_block(
            block1.clone(),
            chain_id.clone(),
            None,
            None,
            None,
            ),
            Err(StateError::ProcessingError {reason}) if reason.contains("No metadata found")
        ));

        // save metadata - can apply
        let block_meta_storage = BlockMetaStorage::new(storage.storage());

        // save applied predecessor
        let block0 = block(0);
        block_meta_storage.put(
            &block0,
            &block_meta_storage::Meta::genesis_meta(&block0, chain_id.as_ref(), true),
        )?;
        // save block
        block_meta_storage.put(
            &block1,
            &block_meta_storage::Meta::new(
                false,
                Some(block0.as_ref().clone()),
                1,
                chain_id.as_ref().clone(),
            ),
        )?;

        // try schedule, false, becuase missing operations
        assert!(matches!(
            data_requester.try_schedule_apply_block(
                block1.clone(),
                chain_id.clone(),
                None,
                None,
                None,
            ),
            Ok(false)
        ));

        // save operations - validation_pass 0 - no operations missing
        OperationsMetaStorage::new(storage.storage()).put(
            &block1,
            &operations_meta_storage::Meta::new(
                0,
                10,
                ChainId::from_base58_check("NetXgtSLGNJvNye")?,
            ),
        )?;

        // try schedule - ok
        let mut peer_queue1 = HashMap::default();
        let mut peer_queue2 = HashMap::default();

        assert!(matches!(
            data_requester.try_schedule_apply_block(
                block1.clone(),
                chain_id.clone(),
                None,
                None,
                Some(&mut peer_queue1),
            ),
            Ok(true)
        ));

        // try schedule twice - not ok
        assert!(matches!(
            data_requester.try_schedule_apply_block(
                block1.clone(),
                chain_id.clone(),
                None,
                None,
                Some(&mut peer_queue1),
            ),
            Ok(false)
        ));

        // try schedule another queue - not ok also (global queue)
        assert!(matches!(
            data_requester.try_schedule_apply_block(
                block1,
                chain_id,
                None,
                None,
                Some(&mut peer_queue2),
            ),
            Ok(false)
        ));

        Ok(())
    }

    fn chain_feeder_mock(
        actor_system: &ActorSystem,
        persistent_storage: PersistentStorage,
    ) -> Result<ChainFeederRef, failure::Error> {
        // run actor's
        let shell_channel =
            ShellChannel::actor(&actor_system).expect("Failed to create shell channel");

        let (block_applier_event_sender, _) = channel();
        let block_applier_run = Arc::new(AtomicBool::new(false));

        actor_system
            .actor_of_props::<ChainFeeder>(
                "mocked_chain_feeder",
                Props::new_args((
                    shell_channel,
                    persistent_storage,
                    Arc::new(Mutex::new(block_applier_event_sender)),
                    block_applier_run,
                    Arc::new(Mutex::new(Some(thread::spawn(|| Ok(()))))),
                )),
            )
            .map_err(|e| e.into())
    }
}
