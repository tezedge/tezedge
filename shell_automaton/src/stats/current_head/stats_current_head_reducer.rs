// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::net::SocketAddr;

use crypto::hash::BlockHash;
use tezos_messages::p2p::encoding::block_header::Level;

use crate::{Action, ActionId, State};

use super::{
    stats_current_head_actions::*, CurrentHeadData, CurrentHeadLevelStats, PeerCurrentHeadData,
};

pub fn stats_current_head_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    match &action.action {
        Action::StatsCurrentHeadReceived(StatsCurrentHeadReceivedAction {
            address,
            level,
            hash,
            block_timestamp,
            receive_timestamp,
            empty_mempool,
        }) if *empty_mempool => {
            add_current_head_time(
                state,
                "received_time",
                *address,
                *level,
                hash.clone(),
                *receive_timestamp,
            );
            state
                .stats
                .current_head
                .level_stats
                .get_mut(&level)
                .map(|level_stats| {
                    level_stats
                        .head_stats
                        .entry(hash.clone())
                        .or_insert(CurrentHeadData {
                            block_timestamp: *block_timestamp,
                            received_timestamp: (*receive_timestamp).into(),
                            baker: None,
                            priority: None,
                            times: HashMap::default(),
                        })
                });
        }
        Action::StatsCurrentHeadPrecheckSuccess(StatsCurrentHeadPrecheckSuccessAction {
            hash,
            baker,
            priority,
        }) => {
            let level = if let Some(level) = state.current_heads.candidate_level() {
                level
            } else {
                return;
            };
            state
                .stats
                .current_head
                .level_stats
                .get_mut(&level)
                .map(|level_stats| {
                    if let Some(stats) = level_stats.head_stats.get_mut(hash) {
                        stats.baker = Some(baker.clone());
                        stats.priority = Some(*priority);
                        stats.times.insert("precheck_start".to_string(), 0);
                        stats.times.insert(
                            "precheck_end".to_string(),
                            action
                                .id
                                .duration_since(level_stats.first_action)
                                .as_nanos() as u64,
                        );
                    }
                });
        }
        Action::StatsCurrentHeadPrepareSend(StatsCurrentHeadPrepareSendAction {
            address,
            level,
            hash,
            empty_mempool,
        }) if *empty_mempool => {
            state
                .stats
                .current_head
                .pending_messages
                .insert(*address, (*level, hash.clone()));
        }
        Action::StatsCurrentHeadSent(StatsCurrentHeadSentAction { address, timestamp }) => {
            if let Some((level, hash)) = state.stats.current_head.pending_messages.remove(address) {
                state
                    .stats
                    .current_head
                    .level_stats
                    .get_mut(&level)
                    .map(|level_stats| {
                        if let Some(stats) = level_stats.head_stats.get_mut(&hash) {
                            let time = timestamp
                                .duration_since(level_stats.first_action)
                                .as_nanos() as u64;
                            stats.times.entry("send_start".to_string()).or_insert(time);
                            stats.times.insert("send_end".to_string(), time);
                        }
                    });
                add_current_head_time(state, "sent_time", *address, level, hash, *timestamp);
            }
        }
        Action::StatsCurrentHeadSentError(StatsCurrentHeadSentErrorAction { address }) => {
            state.stats.current_head.pending_messages.remove(address);
        }
        Action::StatsCurrentHeadPrune(StatsCurrentHeadPruneAction { .. }) => {
            state.stats.current_head.level_stats.retain(|_, stats| {
                action.id.duration_since(stats.last_action) < super::PRUNE_PERIOD
            });
            state.stats.current_head.last_pruned = Some(action.id);
        }

        _ => (),
    }
}

fn add_current_head_time(
    state: &mut State,
    time: &str,
    address: SocketAddr,
    level: Level,
    hash: BlockHash,
    timestamp: ActionId,
) {
    let level_stats = state
        .stats
        .current_head
        .level_stats
        .entry(level)
        .or_insert_with(|| CurrentHeadLevelStats {
            first_address: address,
            first_action: timestamp,
            last_action: timestamp,
            head_stats: Default::default(),
            peer_stats: Default::default(),
        });
    level_stats.last_action = timestamp;
    let first_action = level_stats.first_action;

    let peers = &state.peers;
    let peer_stats = level_stats
        .peer_stats
        .entry(address)
        .or_insert_with(|| PeerCurrentHeadData {
            node_id: peers
                .get(&address)
                .and_then(|peer| peer.status.as_handshaked())
                .map(|hs| &hs.public_key_hash)
                .cloned(),
            hash,
            times: HashMap::new(),
        });

    peer_stats
        .times
        .entry(time.to_string())
        .or_insert_with(|| timestamp.duration_since(first_action).as_nanos() as u64);
}
