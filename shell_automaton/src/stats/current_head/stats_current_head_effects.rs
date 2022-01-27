// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, net::SocketAddr};

use crypto::{
    hash::{BlockHash, CryptoboxPublicKeyHash},
    PublicKeyWithHash,
};
use tezos_messages::{
    base::signature_public_key::SignaturePublicKeyHash,
    p2p::{
        binary_message::MessageHash,
        encoding::{block_header::Level, peer::PeerMessage},
    },
};

use crate::{
    block_applier::BlockApplierApplyState,
    current_head::current_head_actions::CurrentHeadPrecheckSuccessAction,
    peer::message::write::{
        PeerMessageWriteErrorAction, PeerMessageWriteInitAction, PeerMessageWriteSuccessAction,
    },
    rights::{rights_actions::RightsGetAction, RightsKey},
    service::RpcService,
    stats::current_head::CurrentHeadData,
    Action,
};

use super::stats_current_head_actions::*;

// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub fn stats_current_head_effects<S>(store: &mut crate::Store<S>, action: &crate::ActionWithMeta)
where
    S: crate::Service,
{
    match &action.action {
        // Action::CurrentHeadReceived is triggered by current_head effect
        Action::CurrentHeadPrecheckSuccess(CurrentHeadPrecheckSuccessAction {
            block_hash,
            baker,
            priority,
        }) => {
            store.dispatch(StatsCurrentHeadPrecheckSuccessAction {
                hash: block_hash.clone(),
                baker: baker.clone(),
                priority: *priority,
            });
        }
        Action::PeerMessageWriteInit(PeerMessageWriteInitAction { address, message }) => {
            let current_head = if let PeerMessage::CurrentHead(current_head) = message.message() {
                current_head
            } else {
                return;
            };

            let current_block_header = current_head.current_block_header();
            let hash = match current_block_header.message_typed_hash() {
                Ok(v) => v,
                Err(_) => return,
            };

            store.dispatch(StatsCurrentHeadPrepareSendAction {
                address: *address,
                level: current_block_header.level(),
                hash,
                empty_mempool: current_head.current_mempool().is_empty(),
            });
        }
        Action::PeerMessageWriteSuccess(PeerMessageWriteSuccessAction { address }) => {
            store.dispatch(StatsCurrentHeadSentAction {
                address: *address,
                timestamp: action.id,
            });
        }
        Action::PeerMessageWriteError(PeerMessageWriteErrorAction { address, .. }) => {
            store.dispatch(StatsCurrentHeadSentErrorAction { address: *address });
        }
        Action::StatsCurrentHeadRpcGet(StatsCurrentHeadRpcGetAction { rpc_id, level }) => {
            #[derive(Debug, serde::Serialize)]
            struct Stats {
                block_level: Level,
                current_heads: Vec<CurrentHeadStat>,
            }
            #[derive(Debug, serde::Serialize)]
            struct CurrentHeadStat {
                address: SocketAddr,
                node_id: Option<CryptoboxPublicKeyHash>,
                block_hash: BlockHash,
                baker: Option<SignaturePublicKeyHash>,
                baker_priority: Option<u16>,
                #[serde(flatten)]
                times: HashMap<String, u64>,
            }
            let block_application_stats = store
                .service
                .statistics()
                .and_then(|stats| stats.block_stats_get_by_level(*level).cloned());

            let rpc = store.service.rpc();
            store
                .state
                .get()
                .stats
                .current_head
                .level_stats
                .get(level)
                .and_then(|level_stats| {
                    let min_time = u64::from(level_stats.first_action);

                    let current_heads = level_stats
                        .peer_stats
                        .iter()
                        .map(|(peer, stats)| {
                            let mut times = stats.times.clone();
                            let head_data = level_stats.head_stats.get(&stats.hash);
                            head_data.map(|hd| {
                                times.insert("prechecked_time".to_string(), hd.prechecked_time)
                            });
                            block_application_stats.as_ref().map(|s| {
                                times.insert("download_data_start".to_owned(), 0);
                                times.insert(
                                    "download_data_end".to_owned(),
                                    s.load_data_start
                                        .and_then(|t| t.checked_sub(min_time))
                                        .unwrap_or(0),
                                );

                                times.insert(
                                    "load_data_start".to_owned(),
                                    s.load_data_start
                                        .and_then(|t| t.checked_sub(min_time))
                                        .unwrap_or(0),
                                );
                                times.insert(
                                    "load_data_end".to_owned(),
                                    s.load_data_end
                                        .and_then(|t| t.checked_sub(min_time))
                                        .unwrap_or(0),
                                );

                                times.insert(
                                    "apply_block_start".to_owned(),
                                    s.apply_block_start
                                        .and_then(|t| t.checked_sub(min_time))
                                        .unwrap_or(0),
                                );
                                times.insert(
                                    "apply_block_end".to_owned(),
                                    s.apply_block_end
                                        .and_then(|t| t.checked_sub(min_time))
                                        .unwrap_or(0),
                                );

                                times.insert(
                                    "store_result_start".to_owned(),
                                    s.store_result_start
                                        .and_then(|t| t.checked_sub(min_time))
                                        .unwrap_or(0),
                                );
                                times.insert(
                                    "store_result_end".to_owned(),
                                    s.store_result_end
                                        .and_then(|t| t.checked_sub(min_time))
                                        .unwrap_or(0),
                                );
                            });
                            CurrentHeadStat {
                                address: *peer,
                                node_id: stats.node_id.clone(),
                                block_hash: stats.hash.clone(),
                                baker: head_data
                                    .map(CurrentHeadData::baker)
                                    .and_then(|b| b.pk_hash().map_or(None, Some)),
                                baker_priority: head_data.map(CurrentHeadData::priority).cloned(),
                                times,
                            }
                        })
                        .collect::<Vec<_>>();
                    rpc.respond(
                        *rpc_id,
                        Stats {
                            block_level: *level,
                            current_heads,
                        },
                    );
                    Some(())
                })
                .or_else(|| {
                    store.service.rpc().respond(
                        *rpc_id,
                        serde_json::json!({ "error": format!("No stats for level `{level}`") }),
                    );
                    None
                });
        }

        Action::BlockApplierApplySuccess(_) => {
            if !store.state.get().is_bootstrapped() {
                return;
            }
            let block = match &store.state().block_applier.current {
                BlockApplierApplyState::Success { block, .. } => block,
                _ => return,
            };

            let current_block_hash = block.hash.clone();
            let level = block.header.level();
            store.dispatch(RightsGetAction {
                key: RightsKey::baking(current_block_hash, Some(level + 1), None),
            });
            store.dispatch(StatsCurrentHeadPruneAction {
                timestamp: action.id,
            });
        }

        _ => (),
    }
}
