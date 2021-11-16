// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use slog::Logger;

use crypto::hash::OperationHash;
use networking::PeerId;
use shell_integration::MempoolOperationRef;
use tezos_messages::p2p::encoding::limits;
use tezos_messages::p2p::encoding::prelude::{GetOperationsMessage, Mempool, OperationMessage};

use crate::shell_automaton_manager::ShellAutomatonSender;
use crate::state::data_requester::tell_peer;
use crate::state::peer_state::PeerState;

pub enum MempoolOperationStatus {
    /// .0 - contains time of change state datetime
    /// .1 - set of peers, which has the operation
    Missing(Instant, HashSet<Arc<PeerId>>),
    /// .0 - contains time of change state datetime
    /// .1 - set of peers, which has the operation
    /// .2 - set of peers, which were requested to download operation
    ///    - [Option::Some] means, already requested at time
    ///    - [Option::None] means, not requested yet
    Requested(Instant, HashMap<Arc<PeerId>, Option<Instant>>),
    /// .0 - contains time of change state datetime
    Downloaded(Instant),
    /// .0 - contains time of change state datetime
    SentToMempool(Instant, MempoolOperationRef),
    /// .0 - contains time of change state datetime
    AdvertisedToP2p(Instant, MempoolOperationRef),
}

impl MempoolOperationStatus {
    pub fn is_timeouted(&self, max_ttl: &Duration) -> bool {
        match self {
            MempoolOperationStatus::Missing(changed, _)
            | MempoolOperationStatus::Requested(changed, _)
            | MempoolOperationStatus::Downloaded(changed)
            | MempoolOperationStatus::SentToMempool(changed, _)
            | MempoolOperationStatus::AdvertisedToP2p(changed, _) => changed.elapsed().gt(max_ttl),
        }
    }

    pub fn is_advertised(&self) -> bool {
        match self {
            MempoolOperationStatus::Missing(..) => false,
            MempoolOperationStatus::Requested(..) => false,
            MempoolOperationStatus::Downloaded(..) => false,
            MempoolOperationStatus::SentToMempool(..) => false,
            MempoolOperationStatus::AdvertisedToP2p(..) => true,
        }
    }
}

pub enum MempoolDownloadedOperationResult {
    Accept(MempoolOperationRef),
    Ignore,
    Unexpected,
}

#[derive(Clone, Debug)]
pub struct MempoolOperationStateConfiguration {
    /// configured timeout as max_ttl - means, if operation is in state without change, it will be removed
    max_ttl: Duration,
    /// timeout for download one operation (if exceeded, we can try to reschedule to a next peer)
    download_timeout: Duration,
}

impl MempoolOperationStateConfiguration {
    pub fn new(max_ttl: Duration, download_timeout: Duration) -> Self {
        Self {
            max_ttl,
            download_timeout,
        }
    }
}

pub struct MempoolOperationState {
    operations: HashMap<OperationHash, MempoolOperationStatus>,
    cfg: MempoolOperationStateConfiguration,
}

impl MempoolOperationState {
    pub fn new(cfg: MempoolOperationStateConfiguration) -> Self {
        Self {
            operations: HashMap::default(),
            cfg,
        }
    }

    pub fn display_stats(&self) -> String {
        let mut missing = 0;
        let mut requested = 0;
        let mut downloaded = 0;
        let mut sent_to_mempool = 0;
        let mut advertised_to_p2p = 0;

        for op in self.operations.values() {
            match op {
                MempoolOperationStatus::Missing(..) => missing += 1,
                MempoolOperationStatus::Requested(..) => requested += 1,
                MempoolOperationStatus::Downloaded(..) => downloaded += 1,
                MempoolOperationStatus::SentToMempool(..) => sent_to_mempool += 1,
                MempoolOperationStatus::AdvertisedToP2p(..) => advertised_to_p2p += 1,
            }
        }

        format!(
            "Missing({}), Requested({}), Downloaded({}), SentToMempool({}), AdvertisedToP2p({})",
            missing, requested, downloaded, sent_to_mempool, advertised_to_p2p
        )
    }

    pub fn find_mempool_operation(
        &self,
        operation_hash: &OperationHash,
    ) -> Option<&MempoolOperationRef> {
        match self.operations.get(operation_hash) {
            Some(MempoolOperationStatus::SentToMempool(_, mempool_operation)) => {
                Some(mempool_operation)
            }
            Some(MempoolOperationStatus::AdvertisedToP2p(_, mempool_operation)) => {
                Some(mempool_operation)
            }
            _ => None,
        }
    }

    /// Returns Option::Some(download_timeout), which means, that we scheduled something and want to check in [download_timeout] if that succecced or needs to reschedule
    pub fn add_missing_mempool_operations_for_download(
        &mut self,
        received_mempool: &Mempool,
        peer: &Arc<PeerId>,
    ) -> Option<Duration> {
        if received_mempool.is_empty() {
            return None;
        }

        let mut add_to_missing_or_requested =
            |received_operation_hash: &OperationHash, missing_anything: &mut bool| match self
                .operations
                .get_mut(received_operation_hash)
            {
                Some(status) => match status {
                    MempoolOperationStatus::Missing(_, ref mut peers) => {
                        if !peers.contains(peer) {
                            let _ = peers.insert(peer.clone());
                        }
                        *missing_anything = true;
                    }
                    MempoolOperationStatus::Requested(_, ref mut peers) => {
                        if !peers.contains_key(peer) {
                            let _ = peers.insert(peer.clone(), None);
                        }
                        *missing_anything = true;
                    }
                    _ => (),
                },
                None => {
                    let _ = self.operations.insert(
                        received_operation_hash.clone(),
                        MempoolOperationStatus::Missing(
                            Instant::now(),
                            HashSet::from_iter(vec![peer.clone()]),
                        ),
                    );
                    *missing_anything = true;
                    ()
                }
            };

        let mut missing_anything = false;

        let Mempool {
            known_valid,
            pending,
        } = received_mempool;
        known_valid.iter().for_each(|received_operation_hash| {
            add_to_missing_or_requested(received_operation_hash, &mut missing_anything)
        });
        pending.iter().for_each(|received_operation_hash| {
            add_to_missing_or_requested(received_operation_hash, &mut missing_anything)
        });

        if missing_anything {
            Some(self.cfg.download_timeout)
        } else {
            None
        }
    }

    /// Returns Option::Some(download_timeout), which means, that we scheduled something and want to check in [download_timeout] if that succecced or needs to reschedule
    pub fn process_downloading(
        &mut self,
        peers_states: &mut HashMap<SocketAddr, PeerState>,
        shell_automaton: &ShellAutomatonSender,
        log: &Logger,
    ) -> Option<Duration> {
        let Self { operations, cfg } = self;

        // remove all timeouted
        operations.retain(|_, operation_state| !operation_state.is_timeouted(&cfg.max_ttl));

        // callback handles
        let prepare_request_to_send =
            |operation_hash: OperationHash,
             peer: &Arc<PeerId>,
             peer_state: &mut PeerState,
             requests: &mut HashMap<Arc<PeerId>, HashSet<OperationHash>>| {
                // add to send requests
                if let Some(peer_requests) = requests.get_mut(peer) {
                    peer_requests.insert(operation_hash.clone());
                } else {
                    requests.insert(
                        peer.clone(),
                        HashSet::from_iter(vec![operation_hash.clone()]),
                    );
                }

                // add to peer queue
                peer_state.queued_mempool_operations.insert(operation_hash);
                peer_state.mempool_operations_request_last = Some(Instant::now());
            };
        // callback for reschedule operation
        let handle_reschedule_requests =
            |operation_hash: &OperationHash,
             peers_to_reschedule: Vec<(&Arc<PeerId>, &mut Option<Instant>)>,
             closure_peers_states: &mut HashMap<SocketAddr, PeerState>,
             selected_count: &mut usize,
             max_count: usize,
             requests: &mut HashMap<Arc<PeerId>, HashSet<OperationHash>>| {
                for (peer, last_scheduled) in peers_to_reschedule {
                    // check if we can use peer for scheduling
                    if let Some(peer_state) = closure_peers_states.get_mut(&peer.address) {
                        if *selected_count < max_count
                            && peer_state.available_mempool_operations_queue_capacity() > 0
                        {
                            // add to requests
                            prepare_request_to_send(
                                operation_hash.clone(),
                                peer,
                                peer_state,
                                requests,
                            );

                            // increment counter
                            *selected_count += 1;

                            // change date
                            *last_scheduled = Some(Instant::now());
                        }
                    }
                }
            };

        // prepare requests for download operations
        let mut missing_anything = false;
        let mut next_download_timeout_check = cfg.download_timeout;
        let mut request_to_send: HashMap<Arc<PeerId>, HashSet<OperationHash>> = HashMap::new();

        let mut rng = rand::thread_rng();
        let mut random_max_count = || -> usize {
            // select some random peers: 2 or 3 or 4, we better ask more then one peer, just to be sure,
            // that we receive operations as soon as possible without timeout
            rng.gen_range(2, 5)
        };

        operations
            .iter_mut()
            .for_each(|(operation_hash, operation_state)| {
                match operation_state {
                    MempoolOperationStatus::Missing(_, peers) => {
                        missing_anything = true;

                        let max_count = random_max_count();
                        let mut selected_count = 0;

                        // prepare requests (peers is a hashset)
                        let requested_peers: HashMap<Arc<PeerId>, Option<Instant>> = peers
                            .drain()
                            .filter_map(|peer| {
                                // check if we can use peer for scheduling
                                if let Some(peer_state) = peers_states.get_mut(&peer.address) {
                                    if selected_count < max_count
                                        && peer_state.available_mempool_operations_queue_capacity()
                                            > 0
                                    {
                                        // add to requests
                                        prepare_request_to_send(
                                            operation_hash.clone(),
                                            &peer,
                                            peer_state,
                                            &mut request_to_send,
                                        );

                                        // increment counter
                                        selected_count += 1;

                                        // add to state map
                                        Some((peer, Some(Instant::now())))
                                    } else {
                                        // we just add peer for later use
                                        Some((peer, None))
                                    }
                                } else {
                                    // remove from peers, because we dont have state for peer
                                    None
                                }
                            })
                            .collect();

                        // state transition
                        *operation_state =
                            MempoolOperationStatus::Requested(Instant::now(), requested_peers);
                    }
                    MempoolOperationStatus::Requested(_, requested_peers) => {
                        missing_anything = true;

                        let max_count = random_max_count();
                        let mut selected_count = 0;

                        // split potential peers with this operation to "available", which can be used for scheduling and those which are waiting
                        let (never_scheduled, timeouted): (
                            Vec<(&Arc<PeerId>, &mut Option<Instant>)>,
                            Vec<(&Arc<PeerId>, &mut Option<Instant>)>,
                        ) = requested_peers
                            .iter_mut()
                            .filter(|(_, last_requested)| {
                                // filter out waiting
                                match last_requested {
                                    Some(last_requested) => {
                                        if last_requested.elapsed() < cfg.download_timeout {
                                            // but, we want to check next time as soon as possible, so we trying to find lowest next check timeout
                                            if let Some(potential_next_download_check) = cfg
                                                .download_timeout
                                                .checked_sub(last_requested.elapsed())
                                            {
                                                if potential_next_download_check
                                                    < next_download_timeout_check
                                                {
                                                    next_download_timeout_check =
                                                        potential_next_download_check;
                                                }
                                            }
                                            // do nothing, just waiting
                                            false
                                        } else {
                                            // timeouted, lets use it just in case
                                            true
                                        }
                                    }
                                    None => {
                                        // not scheduled, lets use it
                                        true
                                    }
                                }
                            })
                            .partition(|(_, last_requested)| {
                                // split to "never_scheduled" and "timeouted"
                                last_requested.is_none()
                            });

                        // lets try schedule "never_scheduled" at first
                        handle_reschedule_requests(
                            operation_hash,
                            never_scheduled,
                            peers_states,
                            &mut selected_count,
                            max_count,
                            &mut request_to_send,
                        );

                        // if we dont have enought, we try to schedule timeouted again
                        // lets try schedule "never_scheduled" at first
                        if selected_count < max_count {
                            handle_reschedule_requests(
                                operation_hash,
                                timeouted,
                                peers_states,
                                &mut selected_count,
                                max_count,
                                &mut request_to_send,
                            );
                        }
                    }
                    _ => (),
                }
            });

        // send prepared requests to the p2p
        for (peer, operations_to_download) in request_to_send {
            let operations_to_download = Vec::from_iter(operations_to_download);
            if limits::GET_OPERATIONS_MAX_LENGTH > 0 {
                operations_to_download
                    .chunks(limits::GET_OPERATIONS_MAX_LENGTH)
                    .for_each(|operations_to_download| {
                        tell_peer(
                            shell_automaton,
                            &peer,
                            GetOperationsMessage::new(operations_to_download.into()).into(),
                            log,
                        );
                    });
            } else {
                tell_peer(
                    shell_automaton,
                    &peer,
                    GetOperationsMessage::new(operations_to_download).into(),
                    log,
                );
            }
        }

        if missing_anything {
            Some(next_download_timeout_check)
        } else {
            None
        }
    }

    pub fn register_operation_downloaded(
        &mut self,
        operation_hash: &OperationHash,
        operation: &OperationMessage,
        peer: &mut PeerState,
    ) -> MempoolDownloadedOperationResult {
        if let Some(operation_state) = self.operations.get_mut(operation_hash) {
            match operation_state {
                MempoolOperationStatus::Missing(_, _) => {
                    // this should not happen, we need to at first requested it, somebody could sent it without requesting?
                    MempoolDownloadedOperationResult::Ignore
                }
                MempoolOperationStatus::Requested(_, _) => {
                    if peer.queued_mempool_operations.remove(operation_hash) {
                        peer.mempool_operations_response_last = Some(Instant::now());
                        *operation_state = MempoolOperationStatus::Downloaded(Instant::now());
                        MempoolDownloadedOperationResult::Accept(Arc::new(operation.clone()))
                    } else {
                        MempoolDownloadedOperationResult::Unexpected
                    }
                }
                MempoolOperationStatus::Downloaded(..) => {
                    // already downloaded and (probably) processed before
                    MempoolDownloadedOperationResult::Ignore
                }
                MempoolOperationStatus::SentToMempool(..) => {
                    MempoolDownloadedOperationResult::Ignore
                }
                MempoolOperationStatus::AdvertisedToP2p(..) => {
                    MempoolDownloadedOperationResult::Ignore
                }
            }
        } else {
            if peer.queued_mempool_operations.remove(operation_hash) {
                peer.mempool_operations_response_last = Some(Instant::now());
                MempoolDownloadedOperationResult::Ignore
            } else {
                MempoolDownloadedOperationResult::Unexpected
            }
        }
    }

    pub(crate) fn register_sent_to_mempool(
        &mut self,
        operation_hash: &OperationHash,
        downloaded_operation: MempoolOperationRef,
    ) {
        if let Some(operation_state) = self.operations.get_mut(operation_hash) {
            *operation_state =
                MempoolOperationStatus::SentToMempool(Instant::now(), downloaded_operation);
        }
    }

    pub(crate) fn register_advertised_to_p2p(
        &mut self,
        mut mempool: Mempool,
        mempool_operations: HashMap<OperationHash, MempoolOperationRef>,
    ) -> Mempool {
        if mempool.is_empty() || mempool_operations.is_empty() {
            return Mempool::default();
        }

        let mut mark_advertised = |operation_hash: &OperationHash| {
            if let Some(operation_state) = self.operations.get_mut(operation_hash) {
                if !operation_state.is_advertised() {
                    if let Some(mempool_operation) = mempool_operations.get(operation_hash) {
                        *operation_state = MempoolOperationStatus::AdvertisedToP2p(
                            Instant::now(),
                            mempool_operation.clone(),
                        );
                        true
                    } else {
                        // do not propagate OperationHash without stored operation
                        false
                    }
                } else {
                    true
                }
            } else {
                // this could happen, when we inject operation from rpc
                if let Some(mempool_operation) = mempool_operations.get(operation_hash) {
                    let _ = self.operations.insert(
                        operation_hash.clone(),
                        MempoolOperationStatus::AdvertisedToP2p(
                            Instant::now(),
                            mempool_operation.clone(),
                        ),
                    );
                    true
                } else {
                    // do not propagate OperationHash without stored operation
                    false
                }
            }
        };

        let Mempool {
            known_valid,
            pending,
        } = &mut mempool;

        known_valid.retain(|operation_hash| mark_advertised(operation_hash));
        pending.retain(|operation_hash| mark_advertised(operation_hash));

        mempool
    }

    pub fn clear_peer_data(&mut self, peer_to_clear: &SocketAddr) {
        self.operations
            .retain(|_, operation_state| match operation_state {
                MempoolOperationStatus::Missing(_, peers) => {
                    // remove peers
                    peers.retain(|peer| peer.address.ne(peer_to_clear));
                    // if no peers, then remove operation
                    !peers.is_empty()
                }
                MempoolOperationStatus::Requested(_, peers) => {
                    // remove peers
                    peers.retain(|peer, _| peer.address.ne(peer_to_clear));
                    // if no peers, then remove operation
                    !peers.is_empty()
                }
                _ => true,
            });
    }
}
