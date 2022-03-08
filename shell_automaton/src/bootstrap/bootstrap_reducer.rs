// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    net::SocketAddr,
};

use crypto::{
    hash::BlockHash,
    seeded_step::{Seed, Step},
};
use tezos_messages::p2p::encoding::block_header::Level;

use crate::{bootstrap::PeerIntervalError, Action, ActionWithMeta, State};

use super::{
    BlockWithDownloadedHeader, BootstrapBlockOperationGetState, BootstrapState,
    PeerBlockOperationsGetState, PeerBranch, PeerIntervalCurrentState, PeerIntervalState,
};
pub fn bootstrap_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::BootstrapInit(_) => {
            state.bootstrap = BootstrapState::Init {
                time: action.time_as_nanos(),
            };
        }
        Action::BootstrapPeersConnectPending(_) => {
            state.bootstrap = BootstrapState::PeersConnectPending {
                time: action.time_as_nanos(),
            };
        }
        Action::BootstrapPeersConnectSuccess(_) => {
            state.bootstrap = BootstrapState::PeersConnectSuccess {
                time: action.time_as_nanos(),
            };
        }
        Action::BootstrapPeersMainBranchFindPending(_) => {
            state.bootstrap = BootstrapState::PeersMainBranchFindPending {
                time: action.time_as_nanos(),
                peer_branches: Default::default(),
                block_supporters: Default::default(),
            };
        }
        Action::BootstrapPeerCurrentBranchReceived(content) => {
            let peer = content.peer;
            let branch_header = &content.current_head.header;
            let branch_level = branch_header.level();
            let branch_hash = content.current_head.hash.clone();
            let branch_iter = match peer_branch_with_level_iter(
                state,
                peer,
                branch_level,
                branch_hash.clone(),
                &content.history,
            ) {
                Some(v) => v,
                None => return,
            };

            if !state.can_accept_new_head(&content.current_head) {
                return;
            }

            match &mut state.bootstrap {
                BootstrapState::PeersMainBranchFindPending {
                    peer_branches,
                    block_supporters,
                    ..
                } => {
                    peer_branches.insert(
                        content.peer,
                        PeerBranch {
                            current_head: content.current_head.clone(),
                            history: content.history.clone(),
                        },
                    );
                    IntoIterator::into_iter([
                        (branch_level - 1, branch_header.predecessor().clone()),
                        (branch_level, branch_hash.clone()),
                    ])
                    .filter(|(level, _)| *level >= 0)
                    .for_each(|(level, block_hash)| {
                        block_supporters
                            .entry(block_hash)
                            .or_insert((level, Default::default()))
                            .1
                            .insert(content.peer);
                    });
                }
                BootstrapState::PeersBlockHeadersGetPending {
                    main_chain_last_level,
                    main_chain,
                    peer_intervals,
                    ..
                } => {
                    let current_head = match state.current_head.get() {
                        Some(v) => v,
                        None => return,
                    };
                    let branch = branch_iter.collect::<Vec<_>>();

                    branch
                        .iter()
                        .filter(|(level, _)| level <= main_chain_last_level)
                        .filter(|(level, _)| {
                            *level >= *main_chain_last_level - main_chain.len() as Level
                        })
                        .find(|(level, hash)| {
                            let index = *main_chain_last_level - *level;
                            if let Some(v) = main_chain.get(index as usize) {
                                if &v.block_hash == hash {
                                    peer_intervals.last_mut().map(|p| p.peers.insert(peer));
                                    return true;
                                }
                            }
                            false
                        });

                    branch
                        .into_iter()
                        .filter(|(level, _)| *level > current_head.header.level())
                        .filter(|(level, _)| {
                            *level <= *main_chain_last_level - main_chain.len() as Level
                        })
                        .fold(0, |mut index, (block_level, block_hash)| {
                            while let Some(_) = peer_intervals
                                .iter()
                                .rev()
                                .nth(index + 1)
                                .and_then(|p| p.highest_level())
                                .filter(|l| block_level <= *l)
                            {
                                index += 1;
                            }
                            // interval containing `block_level`
                            let matching_interval =
                                peer_intervals.iter_mut().rev().nth(index).filter(|p| {
                                    let (lowest, highest) = match p.lowest_and_highest_levels() {
                                        Some(v) => v,
                                        None => return false,
                                    };
                                    block_level >= lowest && block_level <= highest
                                });

                            if matching_interval.is_some() {
                                // TODO(zura): compare hash as well.
                                peer_intervals
                                    .iter_mut()
                                    .rev()
                                    .skip(index)
                                    .try_for_each(|p| {
                                        p.peers.insert(peer);
                                        if !p.current.is_finished() {
                                            return None;
                                        }
                                        Some(())
                                    });
                            } else {
                                let index = peer_intervals.len() - index - 1;
                                peer_intervals.insert(
                                    index,
                                    PeerIntervalState {
                                        peers: std::iter::once(peer).collect(),
                                        downloaded: vec![],
                                        current: PeerIntervalCurrentState::idle(
                                            action.time_as_nanos(),
                                            block_level,
                                            block_hash,
                                        ),
                                    },
                                );
                            }

                            index
                        });
                }
                _ => {}
            }
        }
        Action::PeerCurrentHeadUpdate(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockHeadersGetPending {
                main_chain_last_level,
                main_chain_last_hash,
                main_chain,
                peer_intervals,
                ..
            } => {
                let level = content.current_head.header.level();
                let hash = &content.current_head.hash;
                let is_same_chain = if *main_chain_last_level >= level {
                    let index = *main_chain_last_level - level;
                    main_chain
                        .get(index as usize)
                        .filter(|b| &b.block_hash == hash)
                        .is_some()
                } else if main_chain_last_hash == content.current_head.header.predecessor() {
                    true
                } else {
                    false
                };
                if is_same_chain {
                    peer_intervals
                        .last_mut()
                        .map(|p| p.peers.insert(content.address));
                }
            }
            _ => {}
        },
        Action::BootstrapFromPeerCurrentHead(content) => {
            let peers = state
                .peers
                .handshaked_iter()
                .filter(|(_, p)| {
                    let head = match p.current_head.as_ref() {
                        Some(v) => v,
                        None => return false,
                    };

                    if head.header.level() == content.current_head.header.level() {
                        head.hash == content.current_head.hash
                    } else if head.header.level() - 1 == content.current_head.header.level() {
                        head.header.predecessor() == &content.current_head.hash
                    } else {
                        false
                    }
                })
                .map(|(addr, _)| addr)
                .chain(std::iter::once(content.peer))
                .collect();
            state.bootstrap = BootstrapState::PeersBlockHeadersGetPending {
                time: action.time_as_nanos(),
                timeouts_last_check: None,
                main_chain_last_level: content.current_head.header.level(),
                main_chain_last_hash: content.current_head.hash.clone(),
                main_chain: Default::default(),
                peer_intervals: vec![PeerIntervalState {
                    peers,
                    downloaded: Default::default(),
                    current: PeerIntervalCurrentState::Pending {
                        time: action.time_as_nanos(),
                        peer: content.peer,
                        block_level: content.current_head.header.level(),
                        block_hash: content.current_head.hash.clone(),
                    },
                }],
            };
        }
        Action::BootstrapPeersMainBranchFindSuccess(_) => match &mut state.bootstrap {
            BootstrapState::PeersMainBranchFindPending { peer_branches, .. } => {
                let peer_branches = std::mem::take(peer_branches);
                if let Some(main_block) = state
                    .bootstrap
                    .main_block(state.config.peers_bootstrapped_min)
                {
                    state.bootstrap = BootstrapState::PeersMainBranchFindSuccess {
                        time: action.time_as_nanos(),

                        main_block,
                        peer_branches,
                    };
                }
            }
            _ => {}
        },
        Action::BootstrapPeersBlockHeadersGetPending(_) => {
            let current_head = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };

            if let BootstrapState::PeersMainBranchFindSuccess {
                main_block,
                peer_branches,
                ..
            } = &mut state.bootstrap
            {
                let main_block = main_block.clone();
                let missing_levels_count = main_block.0 - current_head.header.level();

                let peer_intervals = std::mem::take(peer_branches)
                    .into_iter()
                    .filter_map(|(peer, branch)| {
                        let branch_level = branch.current_head.header.level();
                        let branch_hash = branch.current_head.hash;
                        Some(
                            peer_branch_with_level_iter(
                                state,
                                peer,
                                branch_level,
                                branch_hash,
                                &branch.history,
                            )?
                            .collect::<Vec<_>>()
                            .into_iter()
                            .map(move |(level, hash)| (peer, level, hash)),
                        )
                    })
                    .flatten()
                    .filter(|(_, level, _)| *level > current_head.header.level())
                    .filter(|(_, level, _)| *level <= main_block.0)
                    .fold(BTreeMap::new(), |mut r, (peer, block_level, block_hash)| {
                        r.entry(block_level)
                            .or_insert((block_hash, BTreeSet::new()))
                            .1
                            .insert(peer);
                        r
                    })
                    .into_iter()
                    .map(|(block_level, (block_hash, peers))| PeerIntervalState {
                        peers,
                        downloaded: vec![],
                        current: PeerIntervalCurrentState::idle(
                            action.time_as_nanos(),
                            block_level,
                            block_hash,
                        ),
                    })
                    .collect();

                state.bootstrap = BootstrapState::PeersBlockHeadersGetPending {
                    time: action.time_as_nanos(),
                    timeouts_last_check: None,
                    main_chain_last_level: main_block.0,
                    main_chain_last_hash: main_block.1,
                    main_chain: VecDeque::with_capacity(missing_levels_count.max(0) as usize),
                    peer_intervals,
                };
            }
        }
        Action::BootstrapPeerBlockHeaderGetPending(content) => {
            state
                .bootstrap
                .peer_next_interval_mut(content.peer)
                .map(|(_, p)| p.current.to_pending(action.time_as_nanos(), content.peer));
        }
        Action::BootstrapPeerBlockHeaderGetTimeout(content) => {
            state
                .bootstrap
                .peer_interval_mut(content.peer, |p| {
                    p.current.is_pending_block_hash_eq(&content.block_hash)
                })
                .map(|(_, p)| {
                    p.peers.remove(&content.peer);
                    p.current.to_timed_out(action.time_as_nanos());
                });
        }
        Action::BootstrapPeerBlockHeaderGetSuccess(content) => {
            state
                .bootstrap
                .peer_interval_mut(content.peer, |p| {
                    p.current.is_pending_block_level_and_hash_eq(
                        content.block.header.level(),
                        &content.block.hash,
                    )
                })
                .map(|(_, p)| {
                    p.current
                        .to_success(action.time_as_nanos(), content.block.clone())
                });
        }
        Action::BootstrapPeerBlockHeaderGetFinish(content) => {
            let log = &state.log;
            let current_head = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };
            let (mut index, block) = match state
                .bootstrap
                .peer_interval(content.peer, |p| p.current.is_success())
                .and_then(|(index, p)| p.current.block().map(|b| (index, b)))
            {
                Some((index, b)) => (index, b.clone()),
                None => return,
            };
            if let BootstrapState::PeersBlockHeadersGetPending {
                main_chain_last_level,
                main_chain,
                peer_intervals,
                ..
            } = &mut state.bootstrap
            {
                peer_intervals[index].current = PeerIntervalCurrentState::idle(
                    action.time_as_nanos(),
                    block.header.level() - 1,
                    block.header.predecessor().clone(),
                );
                peer_intervals[index].downloaded.push((
                    block.header.level(),
                    block.hash.clone(),
                    block.header.validation_pass(),
                    block.header.operations_hash().clone(),
                ));

                if index == 0 {
                    let pred_level = block.header.level() - 1;
                    let pred_hash = block.header.predecessor();
                    let current_level = current_head.header.level();
                    if pred_level == current_level && pred_hash == &current_head.hash {
                        peer_intervals[index]
                            .current
                            .to_finished(action.time_as_nanos(), content.peer);
                    } else if current_level - 1 == pred_level {
                        if pred_hash == current_head.header.predecessor() {
                            // allow current head(1 level) reorg
                            peer_intervals[index]
                                .current
                                .to_finished(action.time_as_nanos(), content.peer);
                        }
                    } else if current_level - 2 == pred_level {
                        let head_pred = state.current_head.get_pred();
                        if head_pred.map_or(false, |head_pred| {
                            pred_hash == head_pred.header.predecessor()
                        }) {
                            // allow current head predecessor(2 level) reorg (MAX)
                            peer_intervals[index]
                                .current
                                .to_finished(action.time_as_nanos(), content.peer);
                        } else {
                            peer_intervals[index].current = PeerIntervalCurrentState::Error {
                                time: action.time_as_nanos(),
                                peer: content.peer,
                                block,
                                error: PeerIntervalError::CementedBlockReorg,
                            };
                        }
                    }
                } else {
                    let pred_index = index - 1;
                    let pred = &peer_intervals[pred_index];

                    match pred
                        .downloaded
                        .first()
                        .map(|(l, h, ..)| (*l, h))
                        .or(pred.current.block_level_with_hash().map(|(l, h)| (l, h)))
                    {
                        Some((pred_level, pred_hash)) => {
                            if pred.current.is_error() {
                                peer_intervals.remove(pred_index);
                                index -= 1;
                            } else if pred_level + 1 > block.header.level() {
                                // TODO(zura): log. Impossible state.
                                peer_intervals.remove(pred_index);
                                index -= 1;
                            } else if pred_level + 1 == block.header.level() {
                                if block.header.predecessor() != pred_hash {
                                    slog::warn!(&log, "Predecessor hash mismatch!";
                                                "block_header" => format!("{:?}", block),
                                                "pred_interval" => format!("{:?}", peer_intervals.get(pred_index - 1)),
                                                "interval" => format!("{:?}", pred),
                                                "next_interval" => format!("{:?}", peer_intervals[index]));
                                    peer_intervals[pred_index].current = PeerIntervalCurrentState::Error {
                                        time: action.time_as_nanos(),
                                        peer: content.peer,
                                        block,
                                        error: PeerIntervalError::NextIntervalsPredecessorHashMismatch,
                                    };
                                } else {
                                    peer_intervals[index]
                                        .current
                                        .to_finished(action.time_as_nanos(), content.peer);
                                }
                            }
                        }
                        None => {
                            slog::warn!(&state.log, "Found empty block header download interval when bootstrapping. Should not happen!";
                                    "pred_interval" => format!("{:?}", peer_intervals.get(pred_index - 1)),
                                    "interval" => format!("{:?}", pred),
                                    "next_interval" => format!("{:?}", peer_intervals[index]));
                            peer_intervals.remove(pred_index);
                            index -= 1;
                        }
                    }
                }

                if peer_intervals[index].current.is_finished() {
                    let peers = peer_intervals[index]
                        .peers
                        .iter()
                        .map(|p| *p)
                        .collect::<Vec<_>>();
                    let rev_index = peer_intervals.len() - index - 1;
                    peer_intervals
                        .iter_mut()
                        .rev()
                        .skip(rev_index + 1)
                        .try_for_each(|p| {
                            p.peers.extend(peers.iter().cloned());
                            if !p.current.is_finished() {
                                return None;
                            }
                            Some(())
                        });
                }

                loop {
                    let main_chain_next_level = *main_chain_last_level - main_chain.len() as Level;
                    let interval = match peer_intervals.last_mut() {
                        Some(v) => v,
                        None => break,
                    };
                    let first_downloaded_level = interval.downloaded.first().map(|(l, ..)| *l);
                    if let Some(first_downloaded_level) = first_downloaded_level {
                        if first_downloaded_level < main_chain_next_level {
                            // TODO(zura): log. Should be impossible.
                            break;
                        }
                    }
                    for (_, block_hash, validation_pass, operations_hash) in interval
                        .downloaded
                        .drain(..)
                        .scan(main_chain_next_level, |main_chain_next_level, v| {
                            if *main_chain_next_level != v.0 {
                                slog::error!(log, "Downloaded BlockHeader level doesn't match next block in the chain. Impossible!";
                                    "expected_level" => *main_chain_next_level,
                                    "found_level" => v.0,
                                    "found_hash" => format!("{:?}", v.1));

                                return None;
                            }
                            *main_chain_next_level -= 1;
                            Some(v)
                        }) {
                            main_chain.push_front(BlockWithDownloadedHeader {
                                peer: interval.current.peer(),
                                block_hash,
                                validation_pass,
                                operations_hash,
                            });
                        }
                    if interval.current.is_finished() {
                        peer_intervals.pop();
                    } else if first_downloaded_level.is_none() {
                        break;
                    }
                }
            }
        }
        Action::BootstrapPeersBlockHeadersGetSuccess(_) => match &mut state.bootstrap {
            BootstrapState::PeersBlockHeadersGetPending {
                main_chain_last_level,
                main_chain,
                ..
            } => {
                state.bootstrap = BootstrapState::PeersBlockHeadersGetSuccess {
                    time: action.time_as_nanos(),
                    chain_last_level: *main_chain_last_level,
                    chain: std::mem::take(main_chain),
                };
            }
            _ => {}
        },
        Action::BootstrapPeersBlockOperationsGetPending(_) => match &mut state.bootstrap {
            BootstrapState::PeersBlockHeadersGetSuccess {
                chain_last_level,
                chain,
                ..
            } => {
                state.bootstrap = BootstrapState::PeersBlockOperationsGetPending {
                    time: action.time_as_nanos(),
                    timeouts_last_check: None,
                    last_level: *chain_last_level,
                    queue: std::mem::take(chain),
                    pending: Default::default(),
                };
            }
            _ => {}
        },
        Action::BootstrapPeerBlockOperationsGetPending(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending {
                queue,
                pending,
                last_level,
                ..
            } => {
                let next_block = match queue.pop_front() {
                    Some(v) => v,
                    None => return,
                };
                let next_block_level = *last_level - queue.len() as i32;

                slog::debug!(&state.log, "Scheduled BlockOperationsGet";
                    "block_level" => next_block_level,
                    "block_hash" => format!("{:?}", next_block.block_hash));

                pending
                    .entry(next_block.block_hash.clone())
                    .or_insert(BootstrapBlockOperationGetState {
                        block_level: next_block_level,
                        validation_pass: next_block.validation_pass,
                        operations_hash: next_block.operations_hash.clone(),
                        peers: Default::default(),
                    })
                    .peers
                    .insert(
                        content.peer,
                        PeerBlockOperationsGetState::Pending {
                            time: action.time_as_nanos(),
                            operations: vec![None; next_block.validation_pass as usize],
                        },
                    );
            }
            _ => {}
        },
        Action::BootstrapPeerBlockOperationsGetTimeout(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                pending
                    .get_mut(&content.block_hash)
                    .and_then(|b| b.peers.get_mut(&content.peer))
                    .map(|p| p.to_timed_out(action.time_as_nanos()));
            }
            _ => {}
        },
        Action::BootstrapPeerBlockOperationsGetRetry(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                let block_state = match pending.get_mut(&content.block_hash) {
                    Some(v) => v,
                    None => return,
                };

                let validation_pass = block_state.validation_pass as usize;
                let peer_state_build = move || PeerBlockOperationsGetState::Pending {
                    time: action.time_as_nanos(),
                    operations: vec![None; validation_pass],
                };

                block_state
                    .peers
                    .entry(content.peer)
                    .and_modify(|p| *p = peer_state_build())
                    .or_insert_with(peer_state_build);
            }
            _ => {}
        },
        Action::BootstrapPeerBlockOperationsReceived(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                pending
                    .get_mut(content.message.operations_for_block().block_hash())
                    .and_then(|b| b.peers.get_mut(&content.peer))
                    .and_then(|p| p.pending_operations_mut())
                    .and_then(|operations| {
                        operations.get_mut(
                            content.message.operations_for_block().validation_pass() as usize
                        )
                    })
                    .map(|v| *v = Some(content.message.clone()));
            }
            _ => {}
        },
        Action::BootstrapPeerBlockOperationsGetSuccess(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                pending
                    .get_mut(&content.block_hash)
                    .and_then(|b| b.peers.iter_mut().find(|(_, p)| p.is_complete()))
                    .map(|(_, p)| {
                        if let Some(ops) = p.pending_operations_mut() {
                            let operations = ops.drain(..).filter_map(|v| v).collect();
                            *p = PeerBlockOperationsGetState::Success {
                                time: action.time_as_nanos(),
                                operations,
                            };
                        }
                    });
            }
            _ => {}
        },
        Action::BootstrapScheduleBlockForApply(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                pending.remove(&content.block_hash);
            }
            _ => {}
        },
        Action::BootstrapPeersBlockOperationsGetSuccess(_) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending { .. } => {
                state.bootstrap = BootstrapState::PeersBlockOperationsGetSuccess {
                    time: action.time_as_nanos(),
                };
            }
            _ => {}
        },
        Action::PeerDisconnected(content) => match &mut state.bootstrap {
            BootstrapState::PeersMainBranchFindPending {
                peer_branches,
                block_supporters,
                ..
            } => {
                peer_branches.remove(&content.address);
                block_supporters.retain(|_, (_, peers)| {
                    peers.remove(&content.address);
                    !peers.is_empty()
                });
            }
            BootstrapState::PeersBlockHeadersGetPending { peer_intervals, .. } => {
                for interval in peer_intervals {
                    interval.peers.remove(&content.address);
                    if interval
                        .current
                        .peer()
                        .filter(|p| p == &content.address)
                        .is_some()
                    {
                        if interval.current.is_idle() || interval.current.is_pending() {
                            if let Some((level, hash)) = interval.current.block_level_with_hash() {
                                let hash = hash.clone();
                                interval.current = PeerIntervalCurrentState::Disconnected {
                                    time: action.time_as_nanos(),
                                    peer: content.address,
                                    block_level: level,
                                    block_hash: hash,
                                }
                            }
                        }
                    }
                }
            }
            BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                pending
                    .iter_mut()
                    .filter_map(|(_, b)| b.peers.get_mut(&content.address))
                    .for_each(|p| {
                        *p = PeerBlockOperationsGetState::Disconnected {
                            time: action.time_as_nanos(),
                        }
                    });
            }
            _ => {}
        },
        Action::BootstrapCheckTimeoutsInit(_) => {
            state
                .bootstrap
                .set_timeouts_last_check(action.time_as_nanos());
        }
        Action::BootstrapError(content) => {
            state.bootstrap = BootstrapState::Error {
                time: action.time_as_nanos(),
                error: content.error.clone(),
            }
        }
        Action::BootstrapFinished(_) => {
            let error = match &state.bootstrap {
                BootstrapState::Error { error, .. } => Some(error.clone()),
                _ => None,
            };
            state.bootstrap = BootstrapState::Finished {
                time: action.time_as_nanos(),
                error,
            };
        }
        _ => {}
    }
}

fn peer_branch_with_level_iter<'a>(
    state: &State,
    peer: SocketAddr,
    branch_current_head_level: Level,
    branch_current_head_hash: BlockHash,
    history: &'a [BlockHash],
) -> Option<impl 'a + Iterator<Item = (Level, BlockHash)>> {
    // Calculate step for branch to associate block hashes
    // in the branch with expected levels.
    let peer_pkh = match state.peer_public_key_hash(peer) {
        Some(v) => v,
        None => return None,
    };
    let seed = Seed::new(peer_pkh, &state.config.identity.peer_id);
    let step = Step::init(&seed, &branch_current_head_hash);

    let level = branch_current_head_level;

    // let iter = history
    //     .iter()
    //     .scan((level, step.clone()), |(level, step), hash| {
    //         *level -= step.next_step();
    //         Some((*level, hash))
    //     });
    // let contents = format!(
    //     "{}\n{}\n{}\n\n{}\n\n{}",
    //     state.config.identity.peer_id().to_base58_check(),
    //     peer_pkh.to_base58_check(),
    //     format!("{branch_current_head_level} {branch_current_head_hash}"),
    //     iter.map(|(level, hash)| format!("{level} {}", hash.to_base58_check()))
    //         .collect::<Vec<_>>()
    //         .join("\n"),
    //     history
    //         .iter()
    //         .map(|hash| hash.to_base58_check())
    //         .collect::<Vec<_>>()
    //         .join("\n"),
    // );
    // std::fs::write(format!("/tmp/tezedge/branch_{}", peer), contents).unwrap();
    let history_len = history.len();
    let iter = history
        .iter()
        .enumerate()
        // skip last element since it's level might not be deterministic.
        .take_while(move |(i, _)| *i < history_len.max(2) - 1)
        .map(|(_, v)| v)
        .scan((level, step), |(level, step), hash| {
            *level -= step.next_step();
            Some((*level, hash.clone()))
        })
        .take_while(|(level, _)| *level >= 0);
    Some(std::iter::once((branch_current_head_level, branch_current_head_hash)).chain(iter))
}
