// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{BlockHash, ChainId};
use std::collections::HashMap;
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::block_applier::BlockApplierApplyState;
use crate::block_applier::BlockApplierEnqueueBlockAction;
use crate::mempool::mempool_actions::{
    BlockInjectAction, MempoolAskCurrentHeadAction, MempoolGetPendingOperationsAction,
    MempoolOperationInjectAction, MempoolRegisterOperationsStreamAction,
    MempoolRpcEndorsementsStatusGetAction,
};
use crate::mempool::OperationKind;
use crate::rights::{rights_actions::RightsRpcGetAction, RightsKey};
use crate::service::rpc_service::{RpcRequest, RpcRequestStream};
use crate::service::{RpcService, Service};
use crate::storage::request::StorageRequestStatus;
use crate::{Action, ActionWithMeta, Store};

use super::rpc_actions::RpcInjectBlockAction;
use super::rpc_actions::RpcRejectOutdatedInjectedBlockAction;
use super::rpc_actions::{
    RpcBootstrappedAction, RpcBootstrappedDoneAction, RpcBootstrappedNewBlockAction,
    RpcMonitorValidBlocksAction, RpcReplyValidBlockAction,
};
use super::BootstrapState;

pub fn rpc_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok((msg, rpc_id)) = store.service().rpc().try_recv_stream() {
                match msg {
                    RpcRequestStream::Bootstrapped => {
                        store.dispatch(RpcBootstrappedAction { rpc_id });
                    }
                    RpcRequestStream::ValidBlocks(query) => {
                        store.dispatch(RpcMonitorValidBlocksAction { query, rpc_id });
                    }
                    RpcRequestStream::GetOperations {
                        applied,
                        refused,
                        branch_delayed,
                        branch_refused,
                        outdated,
                    } => {
                        store.dispatch(MempoolRegisterOperationsStreamAction {
                            rpc_id,
                            applied,
                            refused,
                            branch_delayed,
                            branch_refused,
                            outdated,
                        });
                    }
                }
            }
            while let Ok((msg, rpc_id)) = store.service().rpc().try_recv() {
                match msg {
                    RpcRequest::GetCurrentGlobalState { channel } => {
                        let _ = channel.send(store.state.get().clone());
                        store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::Null);
                    }
                    RpcRequest::GetStorageRequests { channel } => {
                        let state = store.state.get();
                        let now = state.time_as_nanos();
                        let req_iter = state.storage.requests.iter();
                        let pending_requests = req_iter
                            .filter_map(|(req_id, req)| {
                                let time = match &req.status {
                                    StorageRequestStatus::Pending { time, .. } => *time,
                                    _ => return None,
                                };
                                Some(crate::service::rpc_service::StorageRequest {
                                    req_id,
                                    pending_since: time,
                                    pending_for: now - time,
                                    kind: req.payload.kind(),
                                    requestor: req.requestor.clone(),
                                })
                            })
                            .collect();
                        let requests = crate::service::rpc_service::StorageRequests {
                            pending: pending_requests,
                            finished: store.service().statistics().map_or(
                                Default::default(),
                                |stats| {
                                    stats
                                        .storage_requests_finished_all()
                                        .iter()
                                        .rev()
                                        .cloned()
                                        .collect()
                                },
                            ),
                        };
                        let _ = channel.send(requests);
                        store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::Null);
                    }
                    RpcRequest::GetActionKindStats { channel } => {
                        let data = store
                            .service
                            .statistics()
                            .map(|s| {
                                s.action_kind_stats()
                                    .iter()
                                    .map(|(k, v)| (k.to_string(), v.clone()))
                                    .collect::<HashMap<_, _>>()
                                    .into()
                            })
                            .unwrap_or_default();
                        let _ = channel.send(data);
                        store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::Null);
                    }
                    RpcRequest::GetActionKindStatsForBlocks {
                        channel,
                        level_filter,
                    } => {
                        let data = store
                            .service()
                            .statistics()
                            .map(|s| {
                                s.action_kind_stats_for_blocks()
                                    .iter()
                                    .filter(|s| {
                                        level_filter
                                            .as_ref()
                                            .map_or(true, |levels| levels.contains(&s.block_level))
                                    })
                                    .cloned()
                                    .collect()
                            })
                            .unwrap_or_default();
                        let _ = channel.send(data);
                        store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::Null);
                    }
                    RpcRequest::GetActionGraph { channel } => {
                        let data = store
                            .service
                            .statistics()
                            .map(|s| s.action_graph().clone())
                            .unwrap_or_default();
                        let _ = channel.send(data);
                        store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::Null);
                    }

                    RpcRequest::GetMempoolOperationStats {
                        channel,
                        hash_filter,
                    } => {
                        let stats = &store.state().mempool.operation_stats;
                        let stats = if let Some(hashes) = hash_filter {
                            stats
                                .iter()
                                .filter(|(hash, _)| hashes.contains(hash))
                                .map(|(hash, stat)| (hash.clone(), stat.clone()))
                                .collect()
                        } else {
                            stats.clone()
                        };
                        let _ = channel.send(stats);
                    }
                    RpcRequest::GetBlockStats { channel } => {
                        let stats = store.service().statistics();
                        let _ = channel.send(stats.map(|s| s.block_stats_get_all().clone()));
                    }
                    RpcRequest::InjectBlock { block } => {
                        if !store.dispatch(RpcInjectBlockAction {
                            rpc_id,
                            block: block.clone(),
                        }) {
                            let state = store.state();
                            let error = if !state.rpc.injected_blocks.contains_key(&block.hash) {
                                "block is already injected and is in the queue to be applied"
                            } else if state
                                .block_applier
                                .current
                                .block_hash()
                                .map_or(true, |hash| hash != &block.hash)
                            {
                                if state.block_applier.current.is_pending() {
                                    "block is already injected and being applied"
                                } else {
                                    "block is already injected and applied"
                                }
                            } else {
                                "level/fitness of the injected block is lower than the node's current head"
                            };
                            store
                                .service
                                .rpc()
                                .respond(rpc_id, serde_json::Value::String(error.to_string()));
                        }
                    }
                    RpcRequest::InjectOperation {
                        operation,
                        operation_hash,
                        injected,
                    } => {
                        let injected_timestamp = store.monotonic_to_time(injected);
                        store.dispatch(MempoolOperationInjectAction {
                            operation,
                            hash: operation_hash,
                            rpc_id,
                            injected_timestamp,
                        });
                    }
                    RpcRequest::InjectBlockStart {
                        chain_id,
                        block_hash,
                        block_header,
                        injected,
                    } => {
                        let injected_timestamp = store.monotonic_to_time(injected);
                        store.dispatch(BlockInjectAction {
                            chain_id,
                            block_hash,
                            block_header,
                            injected_timestamp,
                        });
                    }
                    RpcRequest::RequestCurrentHeadFromConnectedPeers => {
                        store.dispatch(MempoolAskCurrentHeadAction {});
                        store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::Null);
                    }
                    RpcRequest::MempoolStatus => match &store.state().mempool.running_since {
                        None => store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::Null),
                        Some(()) => store
                            .service()
                            .rpc()
                            .respond(rpc_id, serde_json::Value::String("".to_string())),
                    },
                    RpcRequest::GetPendingOperations => {
                        store.dispatch(MempoolGetPendingOperationsAction { rpc_id });
                    }
                    RpcRequest::GetBakingRights { block_hash, level } => {
                        store.dispatch(RightsRpcGetAction {
                            key: RightsKey::baking(block_hash, level, None),
                            rpc_id,
                        });
                    }
                    RpcRequest::GetEndorsingRights { block_hash, level } => {
                        store.dispatch(RightsRpcGetAction {
                            key: RightsKey::endorsing(block_hash, level),
                            rpc_id,
                        });
                    }
                    RpcRequest::GetEndorsementsStatus { matcher } => {
                        store.dispatch(MempoolRpcEndorsementsStatusGetAction { rpc_id, matcher });
                    }

                    RpcRequest::GetStatsCurrentHeadStats {
                        channel,
                        level,
                        round,
                    } => {
                        let _ = channel.send(
                            store
                                .service
                                .statistics()
                                .map_or(Vec::new(), |s| s.block_stats_get_by_level(level, round)),
                        );
                    }
                    RpcRequest::GetMempooEndrosementsStats { channel } => {
                        let stats = store
                            .state()
                            .mempool
                            .validated_operations
                            .ops
                            .iter()
                            .filter(|(_, op)| {
                                OperationKind::from_operation_content_raw(op.data().as_ref())
                                    .is_consensus_operation()
                            })
                            .map(|(op_hash, _)| op_hash.clone())
                            .filter_map(|op| {
                                store
                                    .state()
                                    .mempool
                                    .operation_stats
                                    .get(&op)
                                    .cloned()
                                    .map(|stat| (op, stat))
                            })
                            .collect();
                        let _ = channel.send(stats);
                    }
                }
            }
        }

        Action::CurrentHeadUpdate(content) => {
            let is_bootstrapped = store.state().is_bootstrapped_strict();
            store.dispatch(RpcBootstrappedNewBlockAction {
                block: content.new_head.hash.clone(),
                timestamp: content.new_head.header.timestamp().into(),
                is_bootstrapped,
            });
            for rpc_id in store
                .state
                .get()
                .rpc
                .valid_blocks
                .requests
                .keys()
                .cloned()
                .collect::<Vec<_>>()
            {
                store.dispatch(RpcReplyValidBlockAction {
                    rpc_id,
                    block: content.new_head.clone(),
                    protocol: content.protocol.clone(),
                    next_protocol: content.next_protocol.clone(),
                });
            }

            enqueue_injected_block(store);
        }

        Action::RpcInjectBlock(_)
        | Action::BlockApplierApplyError(_)
        | Action::BlockApplierApplySuccess(_)
        | Action::BootstrapFinished(_) => {
            enqueue_injected_block(store);
        }

        Action::RpcRejectOutdatedInjectedBlock(content) => {
            store.service.rpc().respond(
                content.rpc_id,
                serde_json::Value::String("injected block is outdated".to_string()),
            );
        }

        Action::RpcBootstrapped(RpcBootstrappedAction { rpc_id }) => {
            if let Some(BootstrapState {
                json,
                is_bootstrapped,
            }) = &store.state.get().rpc.bootstrapped.state
            {
                store
                    .service
                    .rpc()
                    .respond_stream(*rpc_id, Some(json.clone()));
                if *is_bootstrapped {
                    store.dispatch(RpcBootstrappedDoneAction {
                        rpc_ids: vec![*rpc_id],
                    });
                }
            }
        }
        Action::RpcBootstrappedNewBlock(RpcBootstrappedNewBlockAction { .. }) => {
            let bootstrapped = &store.state.get().rpc.bootstrapped;
            let rpc_ids: Vec<_> = bootstrapped.requests.iter().cloned().collect();
            if let Some(BootstrapState {
                json,
                is_bootstrapped,
            }) = &bootstrapped.state
            {
                for rpc_id in &rpc_ids {
                    store
                        .service
                        .rpc()
                        .respond_stream(*rpc_id, Some(json.clone()));
                }
                if *is_bootstrapped {
                    store.dispatch(RpcBootstrappedDoneAction { rpc_ids });
                }
            }
        }
        Action::RpcBootstrappedDone(RpcBootstrappedDoneAction { rpc_ids }) => {
            for rpc_id in rpc_ids {
                store.service.rpc().respond_stream(*rpc_id, None);
            }
        }

        Action::RpcMonitorValidBlocks(RpcMonitorValidBlocksAction { rpc_id, .. }) => {
            let (block, protocol, next_protocol) = if let BlockApplierApplyState::Success {
                block,
                block_additional_data,
                ..
            } = &store.state.get().block_applier.current
            {
                (
                    (**block).clone(),
                    block_additional_data.protocol_hash.clone(),
                    block_additional_data.next_protocol_hash.clone(),
                )
            } else {
                return;
            };
            store.dispatch(RpcReplyValidBlockAction {
                rpc_id: *rpc_id,
                block,
                protocol,
                next_protocol,
            });
        }
        Action::RpcReplyValidBlock(RpcReplyValidBlockAction { rpc_id, block, .. }) => {
            #[derive(serde::Serialize)]
            struct ValidBlock<'a> {
                chain_id: &'a ChainId,
                hash: &'a BlockHash,
                #[serde(flatten)]
                header: &'a BlockHeader,
            }

            let json = match serde_json::to_value(ValidBlock {
                chain_id: &store.state.get().config.chain_id,
                hash: &block.hash,
                header: block.header.as_ref(),
            }) {
                Ok(json) => json,
                Err(err) => {
                    slog::warn!(store.state.get().log, "Error converting valid block to json"; "error" => slog::FnValue(|_| err.to_string()));
                    return;
                }
            };
            store.service.rpc().respond_stream(*rpc_id, Some(json));
        }

        _ => {}
    }
}

fn enqueue_injected_block<S: Service>(store: &mut Store<S>) {
    let state = store.state();

    if state.block_applier.current.is_pending() || !state.block_applier.queue.is_empty() {
        return;
    }

    if !state.bootstrap.can_inject_block() {
        return;
    }

    let mut blocks = state
        .rpc
        .injected_blocks
        .iter()
        .map(|(_, data)| {
            (
                data.block.hash.clone(),
                data.rpc_id,
                data.block.header.fitness().clone(),
            )
        })
        .collect::<Vec<_>>();

    blocks.sort_by(|(_, _, a), (_, _, b)| b.cmp(a));

    let mut has_accepted = false;
    for (injected_block_hash, rpc_id, _) in blocks {
        let block_hash = injected_block_hash.clone();
        if !store.dispatch(RpcRejectOutdatedInjectedBlockAction { rpc_id, block_hash }) {
            if !has_accepted {
                has_accepted = true;
                store.dispatch(BlockApplierEnqueueBlockAction {
                    injector_rpc_id: Some(rpc_id),
                    block_hash: injected_block_hash.into(),
                });
            }
        }
    }
}
