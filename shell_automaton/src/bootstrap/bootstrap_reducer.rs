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
                        let branch_level = branch.current_head().level();
                        let branch_hash = if main_block.0 == branch.current_head().level() {
                            main_block.1.clone()
                        } else {
                            match branch.current_head().message_typed_hash() {
                                Ok(v) => v,
                                Err(_) => return None,
                            }
                        };
                        Some(
                            peer_branch_with_level_iter(
                                state,
                                peer,
                                branch_level,
                                branch_hash,
                                branch.history(),
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
                        current: PeerIntervalCurrentState::idle(block_level, block_hash),
                    })
                    .collect();

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
                .peer_next_interval_mut(content.peer)
                .map(|(_, p)| p.current.to_pending(content.peer));
        }
        Action::BootstrapPeerBlockHeaderGetSuccess(content) => {
            state
                .bootstrap
                .peer_interval_mut(content.peer, |p| {
                    p.current
                        .is_pending_block_level_eq(content.block.header.level())
                })
                .map(|(_, p)| p.current.to_success(content.block.clone()));
        }
        Action::BootstrapPeerBlockHeaderGetFinish(content) => {
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
                peer_intervals[index].downloaded.push((
                    block.header.level(),
                    block.hash.clone(),
                    block.header.validation_pass(),
                    block.header.operations_hash().clone(),
                ));

                // check if we have finished downloading interval or
                // if this interval reached predecessor. So that
                // pred_interval_level == current_interval_next_level.
                loop {
                    if index <= 0 {
                        let pred_level = block.header.level() - 1;
                        if pred_level <= current_head.header.level() {
                            peer_intervals[index].current.to_finished();
                        }
                        break;
                    }
                    let pred_index = index - 1;
                    let pred = &peer_intervals[pred_index];
                    match pred
                        .downloaded
                        .first()
                        .map(|(l, h, ..)| (*l, h))
                        .or(pred.current.block_level_with_hash().map(|(l, h)| (l, h)))
                    {
                        Some((pred_level, pred_hash)) => {
                            // TODO(zura): check if pred_level > block.header.level()
                            if pred_level + 1 > block.header.level() {
                                dbg!(&state.bootstrap);
                                todo!();
                            }
                            if pred_level + 1 != block.header.level() {
                                break;
                            }
                            if block.header.predecessor() != pred_hash {
                                slog::warn!(&state.log, "Predecessor hash mismatch!";
                                            "block_header" => format!("{:?}", block),
                                            "pred_interval" => format!("{:?}", peer_intervals.get(pred_index - 1)),
                                            "interval" => format!("{:?}", pred),
                                            "next_interval" => format!("{:?}", peer_intervals[index]));
                                todo!("log and remove pred interval, update `index -= 1`, somehow trigger blacklisting a peer.");
                            } else {
                                let pred = &mut peer_intervals[pred_index];
                                if pred.current.is_disconnected()
                                    || (pred.current.is_idle() && pred.peers.is_empty())
                                {
                                    let pred_downloaded = std::mem::take(&mut pred.downloaded);
                                    let interval = &mut peer_intervals[index];
                                    interval.downloaded.extend(pred_downloaded);

                                    peer_intervals.remove(pred_index);
                                    index -= 1;
                                } else {
                                    // We finished interval.
                                    peer_intervals[index].current.to_finished();
                                    break;
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
                    };
                }

                if !peer_intervals[index].current.is_finished() {
                    peer_intervals[index].current = PeerIntervalCurrentState::idle(
                        block.header.level() - 1,
                        block.header.predecessor().clone(),
                    );
                } else {
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
                        .filter(|(l, ..)| *l <= main_chain_next_level)
                    {
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