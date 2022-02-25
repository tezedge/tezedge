// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::action::BootstrapNewCurrentHeadAction;
use crate::block_applier::BlockApplierApplyState;

use crate::mempool::mempool_actions::{
    BlockInjectAction, MempoolAskCurrentHeadAction, MempoolGetPendingOperationsAction,
    MempoolOperationInjectAction, MempoolRegisterOperationsStreamAction,
    MempoolRemoveAppliedOperationsAction, MempoolRpcEndorsementsStatusGetAction,
};
use crate::mempool::OperationKind;
use crate::rights::{rights_actions::RightsRpcGetAction, RightsKey};
use crate::service::rpc_service::{RpcRequest, RpcRequestStream};
use crate::service::{RpcService, Service};
use crate::{Action, ActionWithMeta, Store};

use super::rpc_actions::{
    RpcBootstrappedAction, RpcBootstrappedDoneAction, RpcBootstrappedNewBlockAction,
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
                    RpcRequestStream::GetOperations {
                        applied,
                        refused,
                        branch_delayed,
                        branch_refused,
                    } => {
                        store.dispatch(MempoolRegisterOperationsStreamAction {
                            rpc_id,
                            applied,
                            refused,
                            branch_delayed,
                            branch_refused,
                        });
                    }
                }
            }
            while let Ok((msg, rpc_id)) = store.service().rpc().try_recv() {
                match msg {
                    RpcRequest::GetCurrentGlobalState { channel } => {
                        let _ = channel.send(store.state.get().clone());
                    }
                    RpcRequest::GetMempoolOperationStats { channel } => {
                        let stats = store.state().mempool.operation_stats.clone();
                        let _ = channel.send(stats);
                    }
                    RpcRequest::GetBlockStats { channel } => {
                        let stats = store.service().statistics();
                        let _ = channel.send(stats.map(|s| s.block_stats_get_all().clone()));
                    }
                    RpcRequest::InjectOperation {
                        operation,
                        operation_hash,
                        injected,
                    } => {
                        let injected_timestamp = store.monotonic_to_time(injected);
                        store.dispatch(MempoolOperationInjectAction {
                            operation,
                            operation_hash,
                            rpc_id,
                            injected_timestamp,
                        });
                    }
                    RpcRequest::InjectBlock {
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
                    }
                    RpcRequest::RemoveOperations { operation_hashes } => {
                        store.dispatch(MempoolRemoveAppliedOperationsAction { operation_hashes });
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
                    RpcRequest::GetEndorsementsStatus { block_hash } => {
                        store
                            .dispatch(MempoolRpcEndorsementsStatusGetAction { rpc_id, block_hash });
                    }

                    RpcRequest::GetStatsCurrentHeadStats { channel, level } => {
                        let _ = channel.send(
                            store
                                .service
                                .statistics()
                                .map_or(Vec::new(), |s| s.block_stats_get_by_level(level)),
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
                                OperationKind::from_operation_content_raw(op.data())
                                    .is_endorsement()
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

        Action::BootstrapNewCurrentHead(BootstrapNewCurrentHeadAction {
            chain_id: _,
            block,
            is_bootstrapped,
        }) => {
            store.dispatch(RpcBootstrappedNewBlockAction {
                block: block.hash.clone(),
                timestamp: block.header.timestamp(),
                is_bootstrapped: *is_bootstrapped,
            });
        }
        Action::BlockApplierApplySuccess(_) => {
            if let BlockApplierApplyState::Success { block, .. } =
                &store.state().block_applier.current
            {
                let timestamp = block.header.timestamp();
                let block = block.hash.clone();

                store.dispatch(RpcBootstrappedNewBlockAction {
                    block,
                    timestamp,
                    is_bootstrapped: true,
                });
            }
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
        _ => {}
    }
}
