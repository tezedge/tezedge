// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::VecDeque, net::SocketAddr};

use crypto::{
    hash::BlockHash,
    seeded_step::{Seed, Step},
};
use tezos_messages::p2p::{binary_message::MessageHash, encoding::block_header::Level};

use crate::{Action, ActionWithMeta, State};

use super::{
    BlockWithDownloadedHeader, BootstrapBlockOperationGetState, BootstrapState,
    PeerBlockOperationsGetState, PeerIntervalCurrentState, PeerIntervalState,
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
            let branch = &content.current_branch;
            let branch_level = branch.current_head().level();
            let branch_hash = match branch.current_head().message_typed_hash() {
                Ok(v) => v,
                Err(_) => return,
            };
            let branch_iter = match peer_branch_with_level_iter(
                state,
                peer,
                branch_level,
                branch_hash,
                branch.history(),
            ) {
                Some(v) => v,
                None => return,
            };

            match &mut state.bootstrap {
                BootstrapState::PeersMainBranchFindPending {
                    peer_branches,
                    block_supporters,
                    ..
                } => {
                    peer_branches.insert(content.peer, content.current_branch.clone());
                    let level = content.current_branch.current_head().level();

                    IntoIterator::into_iter([
                        Ok((
                            level - 1,
                            content.current_branch.current_head().predecessor().clone(),
                        )),
                        content
                            .current_branch
                            .current_head()
                            .message_typed_hash()
                            .map(|hash| (level, hash)),
                    ])
                    .filter_map(Result::ok)
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

                    let (_, changed) = branch_iter
                        .filter(|(level, _)| *level > current_head.header.level())
                        .filter(|(level, _)| {
                            *level <= *main_chain_last_level - main_chain.len() as Level
                        })
                        .fold(
                            (peer_intervals.len() - 1, false),
                            |(mut index, changed), (block_level, block_hash)| {
                                loop {
                                    let interval = &peer_intervals[index];
                                    let (lowest_level, highest_level) =
                                        match interval.lowest_and_highest_levels() {
                                            Some(v) => v,
                                            None => {
                                                if index == 0 {
                                                    return (index, changed);
                                                }
                                                index -= 1;
                                                continue;
                                            }
                                        };
                                    if block_level <= highest_level {
                                        if block_level == lowest_level
                                            && interval.current.is_disconnected()
                                        {
                                            peer_intervals[index].current.to_finished();
                                            break;
                                        }
                                        if block_level >= lowest_level {
                                            // no point to add new interval if we are downloading
                                            // or have downloaded this block.
                                            return (index, changed);
                                        }
                                        if index > 0
                                            && peer_intervals[index - 1]
                                                .highest_level()
                                                .filter(|l| block_level <= *l)
                                                .is_some()
                                        {
                                            index -= 1;
                                            continue;
                                        }
                                        break;
                                    }
                                    if index == 0 {
                                        return (index, changed);
                                    }
                                    index -= 1;
                                }

                                peer_intervals.push(PeerIntervalState {
                                    peer,
                                    downloaded: vec![],
                                    current: PeerIntervalCurrentState::Idle {
                                        block_level,
                                        block_hash,
                                    },
                                });
                                (index, true)
                            },
                        );

                    if changed {
                        peer_intervals.sort_by(peer_intervals_cmp_levels);
                    }
                    // We need to make sure that last interval isn't
                    // disconnected as we will be stuck forever in block
                    // headers fetching. So let's replace disconnected
                    // interval with this new peer.
                    if let Some(interval) = peer_intervals.pop() {
                        match interval.current {
                            PeerIntervalCurrentState::Disconnected {
                                block_level,
                                block_hash,
                                ..
                            } => {
                                peer_intervals.push(PeerIntervalState {
                                    peer,
                                    downloaded: vec![],
                                    current: PeerIntervalCurrentState::Idle {
                                        block_level,
                                        block_hash,
                                    },
                                });
                            }
                            current => {
                                peer_intervals.push(PeerIntervalState {
                                    peer: interval.peer,
                                    downloaded: interval.downloaded,
                                    current,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
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
                let mut peer_intervals = vec![];
                let missing_levels_count = main_block.0 - current_head.header.level();

                for (peer, branch) in std::mem::take(peer_branches) {
                    let branch_level = branch.current_head().level();
                    let branch_hash = if main_block.0 == branch.current_head().level() {
                        main_block.1.clone()
                    } else {
                        match branch.current_head().message_typed_hash() {
                            Ok(v) => v,
                            Err(_) => continue,
                        }
                    };
                    let iter = match peer_branch_with_level_iter(
                        state,
                        peer,
                        branch_level,
                        branch_hash,
                        branch.history(),
                    ) {
                        Some(v) => v,
                        None => return,
                    };
                    iter.filter(|(level, _)| *level > current_head.header.level())
                        .filter(|(level, _)| *level <= main_block.0)
                        .for_each(|(block_level, block_hash)| {
                            peer_intervals.push(PeerIntervalState {
                                peer,
                                downloaded: vec![],
                                current: PeerIntervalCurrentState::Idle {
                                    block_level,
                                    block_hash,
                                },
                            });
                        });
                }

                peer_intervals.sort_by(peer_intervals_cmp_levels);
                peer_intervals.dedup_by(|a, b| peer_intervals_cmp_levels(a, b).is_eq());

                state.bootstrap = dbg!(BootstrapState::PeersBlockHeadersGetPending {
                    time: action.time_as_nanos(),
                    main_chain_last_level: main_block.0,
                    main_chain_last_hash: main_block.1,
                    main_chain: VecDeque::with_capacity(missing_levels_count.max(0) as usize),
                    peer_intervals,
                });
            }
        }
        Action::BootstrapPeerBlockHeaderGetPending(content) => {
            state
                .bootstrap
                .peer_interval_mut(content.peer, |p| p.current.is_idle())
                .map(|(_, p)| p.current.to_pending());
        }
        Action::BootstrapPeerBlockHeaderGetSuccess(content) => {
            state
                .bootstrap
                .peer_interval_mut(content.peer, |p| p.current.is_pending())
                .map(|(_, p)| p.current.to_success(content.block.clone()));
        }
        Action::BootstrapPeerBlockHeaderGetFinish(content) => {
            let current_head = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };
            let (index, block) = match state
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
                    block.header.level() - 1,
                    block.header.predecessor().clone(),
                );
                peer_intervals[index].downloaded.push((
                    block.header.level(),
                    block.hash.clone(),
                    block.header.validation_pass(),
                    block.header.operations_hash().clone(),
                ));

                // check if we have finished downloading interval or
                // if this interval reached predecessor. So that
                // pred_interval_level == current_interval_next_level.
                if index > 0 {
                    let pred_index = index - 1;
                    let pred = &peer_intervals[pred_index];
                    match pred
                        .downloaded
                        .first()
                        .map(|(l, h, ..)| (*l, h))
                        .or(pred.current.block_level_with_hash().map(|(l, h)| (l, h)))
                    {
                        Some((pred_level, pred_hash)) => {
                            if pred_level + 1 == block.header.level() {
                                if block.header.predecessor() != pred_hash {
                                    slog::warn!(&state.log, "Predecessor hash mismatch!";
                                        "block_header" => format!("{:?}", block),
                                        "pred_interval" => format!("{:?}", peer_intervals.get(pred_index - 1)),
                                        "interval" => format!("{:?}", pred),
                                        "next_interval" => format!("{:?}", peer_intervals[index]));
                                    todo!("log and remove pred interval, update `index -= 1`, somehow trigger blacklisting a peer.");
                                } else {
                                    // We finished interval.
                                    peer_intervals[index].current.to_finished();
                                }
                            }
                        }
                        None => {
                            slog::warn!(&state.log, "Found empty block header download interval when bootstrapping. Should not happen!";
                                "pred_interval" => format!("{:?}", peer_intervals.get(pred_index - 1)),
                                "interval" => format!("{:?}", pred),
                                "next_interval" => format!("{:?}", peer_intervals[index]));
                            peer_intervals.remove(pred_index);
                        }
                    };
                } else {
                    let pred_level = block.header.level() - 1;
                    if pred_level <= current_head.header.level() {
                        peer_intervals[index].current.to_finished();
                    }
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
                        .filter(|(l, ..)| *l <= main_chain_next_level)
                    {
                        main_chain.push_front(BlockWithDownloadedHeader {
                            peer: interval.peer,
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
                    last_level: *chain_last_level,
                    queue: std::mem::take(chain),
                    pending: Default::default(),
                };
            }
            _ => {}
        },
        Action::BootstrapPeerBlockOperationsGetPending(_) => match &mut state.bootstrap {
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
                        next_block.peer,
                        PeerBlockOperationsGetState::Pending {
                            time: action.time_as_nanos(),
                            operations: vec![None; next_block.validation_pass as usize],
                        },
                    );
            }
            _ => {}
        },
        Action::BootstrapPeerBlockOperationsReceived(content) => match &mut state.bootstrap {
            BootstrapState::PeersBlockOperationsGetPending { pending, .. } => {
                pending
                    .get_mut(content.message.operations_for_block().block_hash())
                    .and_then(|b| b.peers.get_mut(&content.peer))
                    .and_then(|p| match p {
                        PeerBlockOperationsGetState::Pending { operations, .. } => Some(operations),
                        _ => None,
                    })
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
                    .map(|(_, p)| match p {
                        PeerBlockOperationsGetState::Pending { operations, .. } => {
                            let operations = operations.drain(..).filter_map(|v| v).collect();
                            *p = PeerBlockOperationsGetState::Success {
                                time: action.time_as_nanos(),
                                operations,
                            };
                        }
                        _ => {}
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
        Action::PeerDisconnected(content) => {
            match &mut state.bootstrap {
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
                    peer_intervals
                        .iter_mut()
                        .filter(|p| p.peer == content.address)
                        .for_each(|p| p.current.to_disconnected());
                    // TODO(zura): remove all intervals that isn't last.
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
            }
        }
        _ => {}
    }
}

fn peer_intervals_cmp_levels(a: &PeerIntervalState, b: &PeerIntervalState) -> std::cmp::Ordering {
    a.current.block_level().cmp(&b.current.block_level())
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

    let iter = history.iter().scan((level, step), |(level, step), hash| {
        *level -= step.next_step();
        Some((*level, hash.clone()))
    });
    Some(std::iter::once((branch_current_head_level, branch_current_head_hash)).chain(iter))
}
