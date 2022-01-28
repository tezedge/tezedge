// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, net::SocketAddr};

use crypto::{
    hash::{BlockHash, CryptoboxPublicKeyHash},
    PublicKeyWithHash,
};
use tezos_messages::{
    base::signature_public_key::SignaturePublicKeyHash,
    p2p::{binary_message::MessageHash, encoding::peer::PeerMessage},
};

use crate::{
    block_applier::BlockApplierApplyState,
    current_head::current_head_actions::CurrentHeadPrecheckSuccessAction,
    peer::message::write::{
        PeerMessageWriteErrorAction, PeerMessageWriteInitAction, PeerMessageWriteSuccessAction,
    },
    rights::{rights_actions::RightsGetAction, RightsKey},
    service::RpcService,
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
        Action::StatsCurrentHeadRpcGetApplication(StatsCurrentHeadRpcGetApplicationAction {
            rpc_id,
            level,
        }) => {
            #[derive(Debug, serde::Serialize)]
            struct CurrentHeadAppStat {
                block_hash: BlockHash,
                block_timestamp: u64,
                receive_timestamp: u64,
                baker: Option<SignaturePublicKeyHash>,
                baker_priority: Option<u16>,
                #[serde(flatten)]
                times: HashMap<String, u64>,
            }

            let block_application_stats = store
                .service
                .statistics()
                .map(|stats| stats.block_stats_get_all());

            store
                .state
                .get()
                .stats
                .current_head
                .level_stats
                .get(level)
                .map(|level_stats| {
                    let min_time = u64::from(level_stats.first_action);
                    let delta_time = |time: Option<u64>| time.map(|t| t.saturating_sub(min_time));
                    level_stats
                        .head_stats
                        .iter()
                        .map(|(hash, stats)| {
                            let times = block_application_stats
                                .and_then(|bas| bas.get(hash))
                                .map(|bas| {
                                    IntoIterator::into_iter([
                                        ("download_data_start", Some(0)),
                                        ("download_data_end", bas.load_data_start),
                                        ("load_data_start", bas.load_data_start),
                                        ("load_data_end", bas.load_data_end),
                                        ("apply_block_start", bas.apply_block_start),
                                        ("apply_block_end", bas.apply_block_end),
                                        ("store_result_start", bas.store_result_start),
                                        ("store_result_end", bas.store_result_end),
                                    ])
                                    .map(|(k, v)| {
                                        (k.to_string(), delta_time(v).unwrap_or_default())
                                    })
                                    .chain(
                                        bas.apply_block_stats
                                            .as_ref()
                                            .map(|abs| {
                                                [
                                                    ("apply_start", abs.apply_start),
                                                    (
                                                        "operations_decoding_start",
                                                        abs.operations_decoding_start,
                                                    ),
                                                    (
                                                        "operations_decoding_end",
                                                        abs.operations_decoding_end,
                                                    ),
                                                    //("operations_application", ...),
                                                    (
                                                        "operations_metadata_encoding_start",
                                                        abs.operations_metadata_encoding_start,
                                                    ),
                                                    (
                                                        "operations_metadata_encoding_end",
                                                        abs.operations_metadata_encoding_end,
                                                    ),
                                                    (
                                                        "begin_application_start",
                                                        abs.begin_application_start,
                                                    ),
                                                    (
                                                        "begin_application_end",
                                                        abs.begin_application_end,
                                                    ),
                                                    (
                                                        "finalize_block_start",
                                                        abs.finalize_block_start,
                                                    ),
                                                    ("finalize_block_end", abs.finalize_block_end),
                                                    (
                                                        "collect_new_rolls_owner_snapshots_start",
                                                        abs.collect_new_rolls_owner_snapshots_start,
                                                    ),
                                                    (
                                                        "collect_new_rolls_owner_snapshots_end",
                                                        abs.collect_new_rolls_owner_snapshots_end,
                                                    ),
                                                    ("commit_start", abs.commit_start),
                                                    ("commit_end", abs.commit_end),
                                                    ("apply_end", abs.apply_end),
                                                ]
                                                .to_vec()
                                            })
                                            .unwrap_or(Vec::new())
                                            .into_iter()
                                            .map(|(k, v)| {
                                                (
                                                    format!("protocol_{k}"),
                                                    v.saturating_sub(min_time),
                                                )
                                            }),
                                    )
                                    .chain(stats.times.clone())
                                    .collect::<HashMap<_, _>>()
                                })
                                .unwrap_or_default();

                            CurrentHeadAppStat {
                                block_hash: hash.clone(),
                                block_timestamp: stats.block_timestamp,
                                receive_timestamp: stats.received_timestamp,
                                baker: stats
                                    .baker
                                    .as_ref()
                                    .and_then(|b| b.pk_hash().map_or(None, Some)),
                                baker_priority: stats.priority,
                                times,
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .map(|d| {
                    store.service.rpc().respond(*rpc_id, d);
                })
                .unwrap_or_else(|| {
                    store.service.rpc().respond(
                        *rpc_id,
                        serde_json::json!({ "error": format!("No stats for level `{level}`") }),
                    );
                });
        }
        Action::StatsCurrentHeadRpcGetPeers(StatsCurrentHeadRpcGetPeersAction {
            rpc_id,
            level,
        }) => {
            #[derive(Debug, serde::Serialize)]
            struct CurrentHeadStat {
                address: SocketAddr,
                node_id: Option<CryptoboxPublicKeyHash>,
                block_hash: BlockHash,
                #[serde(flatten)]
                times: HashMap<String, u64>,
            }

            store
                .state
                .get()
                .stats
                .current_head
                .level_stats
                .get(level)
                .map(|level_stats| {
                    level_stats
                        .peer_stats
                        .iter()
                        .map(|(peer, stats)| CurrentHeadStat {
                            address: *peer,
                            node_id: stats.node_id.clone(),
                            block_hash: stats.hash.clone(),
                            times: stats.times.clone(),
                        })
                        .collect::<Vec<_>>()
                })
                .map(|d| {
                    store.service.rpc().respond(*rpc_id, d);
                })
                .unwrap_or_else(|| {
                    store.service.rpc().respond(
                        *rpc_id,
                        serde_json::json!({ "error": format!("No stats for level `{level}`") }),
                    );
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
