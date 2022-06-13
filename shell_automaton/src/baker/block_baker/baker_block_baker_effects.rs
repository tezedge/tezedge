// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use storage::BlockHeaderWithHash;
use tezos_api::ffi::ComputePathRequest;
use tezos_messages::p2p::encoding::operations_for_blocks::{
    OperationsForBlock, OperationsForBlocksMessage,
};

use crate::baker::seed_nonce::{BakerSeedNonceCommittedAction, BakerSeedNonceGeneratedAction};
use crate::block_applier::BlockApplierEnqueueBlockAction;
use crate::rights::rights_actions::RightsGetAction;
use crate::rights::RightsKey;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::storage_service::StorageRequestPayload;
use crate::service::{BakerService, ProtocolRunnerService, RandomnessService};
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BakerBlockBakerBakeNextLevelAction, BakerBlockBakerBuildBlockInitAction,
    BakerBlockBakerBuildBlockSuccessAction, BakerBlockBakerComputeOperationsPathsInitAction,
    BakerBlockBakerComputeOperationsPathsPendingAction,
    BakerBlockBakerComputeOperationsPathsSuccessAction,
    BakerBlockBakerComputeProofOfWorkInitAction, BakerBlockBakerComputeProofOfWorkPendingAction,
    BakerBlockBakerInjectInitAction, BakerBlockBakerInjectPendingAction,
    BakerBlockBakerInjectSuccessAction, BakerBlockBakerPreapplyInitAction,
    BakerBlockBakerPreapplyPendingAction, BakerBlockBakerPreapplySuccessAction,
    BakerBlockBakerRightsGetCurrentLevelSuccessAction, BakerBlockBakerRightsGetInitAction,
    BakerBlockBakerRightsGetNextLevelSuccessAction, BakerBlockBakerRightsGetPendingAction,
    BakerBlockBakerRightsGetSuccessAction, BakerBlockBakerRightsNoRightsAction,
    BakerBlockBakerSignInitAction, BakerBlockBakerSignPendingAction, BakerBlockBakerState,
    BakerBlockBakerTimeoutPendingAction, BlockPreapplyRequest,
};

pub fn baker_block_baker_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) | Action::BakerAdd(_) => {
            store.dispatch(BakerBlockBakerRightsGetInitAction {});
        }
        Action::BakerBlockBakerRightsGetInit(_) => {
            let (head_level, head_hash) = match store.state().current_head.get() {
                Some(v) => (v.header.level(), v.hash.clone()),
                None => return,
            };

            let bakers = store.state().baker_keys_iter().cloned().collect::<Vec<_>>();
            for baker in bakers {
                store.dispatch(BakerBlockBakerRightsGetPendingAction { baker });
            }

            store.dispatch(RightsGetAction {
                key: RightsKey::endorsing(head_hash, Some(head_level + 1)),
            });
        }
        Action::RightsValidatorsReady(content) => {
            let head = match store.state().current_head.get() {
                Some(v) => v,
                None => return,
            };
            let is_level_current = content
                .key
                .level()
                .map_or(true, |level| level == head.header.level());
            let is_level_next = content
                .key
                .level()
                .map_or(false, |level| level == head.header.level() + 1);
            if content.key.block() != &head.hash || (!is_level_current && !is_level_next) {
                return;
            }
            let rights_level = if is_level_current {
                head.header.level()
            } else if is_level_next {
                head.header.level() + 1
            } else {
                return;
            };

            let rights = match store.state().rights.tenderbake_validators(rights_level) {
                Some(v) => v,
                None => return,
            };

            let bakers_slots = store
                .state()
                .baker_keys_iter()
                .cloned()
                .map(|baker| {
                    let baker_key = baker.clone();
                    rights
                        .slots
                        .get(&baker)
                        .map(|slots| (baker, slots.clone()))
                        .unwrap_or((baker_key, vec![]))
                })
                .collect::<Vec<_>>();

            for (baker, slots) in bakers_slots {
                if is_level_current {
                    store.dispatch(BakerBlockBakerRightsGetCurrentLevelSuccessAction {
                        baker,
                        slots,
                    });
                } else {
                    store.dispatch(BakerBlockBakerRightsGetNextLevelSuccessAction { baker, slots });
                }
            }
        }
        Action::BakerBlockBakerRightsGetCurrentLevelSuccess(content) => {
            store.dispatch(BakerBlockBakerRightsNoRightsAction {
                baker: content.baker.clone(),
            });
            store.dispatch(BakerBlockBakerRightsGetSuccessAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerRightsGetNextLevelSuccess(content) => {
            store.dispatch(BakerBlockBakerRightsNoRightsAction {
                baker: content.baker.clone(),
            });
            store.dispatch(BakerBlockBakerRightsGetSuccessAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerRightsGetSuccess(content) => {
            store.dispatch(BakerBlockBakerTimeoutPendingAction {
                baker: content.baker.clone(),
            });
        }
        Action::MempoolQuorumReached(_) => {
            let bakers = store.state().baker_keys_iter().cloned().collect::<Vec<_>>();
            for baker in bakers {
                store.dispatch(BakerBlockBakerBakeNextLevelAction { baker });
            }
        }
        Action::BakerBlockBakerBakeNextLevel(content) => {
            store.dispatch(BakerBlockBakerBuildBlockInitAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerBakeNextRound(content) => {
            store.dispatch(BakerBlockBakerBuildBlockInitAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerBuildBlockInit(content) => {
            let head_level = match store.state().current_head.level() {
                Some(v) => v,
                None => return,
            };
            let blocks_per_commitment = match store.state().current_head.constants() {
                Some(v) => v.blocks_per_commitment,
                None => return,
            };
            let level = match store.state().bakers.get(&content.baker) {
                Some(baker) => match &baker.block_baker {
                    BakerBlockBakerState::BakeNextRound { .. } => head_level,
                    BakerBlockBakerState::BakeNextLevel { .. } => head_level + 1,
                    _ => return,
                },
                None => return,
            };
            let seed_nonce_hash = if level % blocks_per_commitment == 0 {
                let (nonce_hash, nonce) = store.service.randomness().get_seed_nonce(level);
                store.dispatch(BakerSeedNonceGeneratedAction {
                    baker: content.baker.clone(),
                    level,
                    nonce,
                    nonce_hash: nonce_hash.clone(),
                });
                Some(nonce_hash)
            } else {
                None
            };
            store.dispatch(BakerBlockBakerBuildBlockSuccessAction {
                baker: content.baker.clone(),
                seed_nonce_hash,
            });
        }
        Action::BakerBlockBakerBuildBlockSuccess(content) => {
            store.dispatch(BakerBlockBakerPreapplyInitAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerPreapplyInit(content) => {
            let baker = match store.state.get().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            let block = match &baker.block_baker {
                BakerBlockBakerState::BuildBlock { block, .. } => block,
                _ => return,
            };
            let protocol_data = match block.bin_encode_protocol_data() {
                Ok(v) => v,
                Err(_) => return,
            };

            let chain_id = store.state.get().config.chain_id.clone();
            let request = BlockPreapplyRequest {
                chain_id,
                protocol_data,
                timestamp: block.timestamp.clone(),
                operations: block.operations.clone(),
                predecessor_header: block.predecessor_header.clone(),
                predecessor_block_metadata_hash: block.pred_block_metadata_hash.clone(),
                predecessor_ops_metadata_hash: block.pred_ops_metadata_hash.clone(),
                predecessor_max_operations_ttl: 120,
            };

            let req_id = store
                .service
                .protocol_runner()
                .preapply_block(request.clone().into());

            slog::debug!(&store.state().log, "Baker block preapply request sent";
                "req_id" => format!("{:?}", req_id),
                "request" => format!("{:?}", request));

            store.dispatch(BakerBlockBakerPreapplyPendingAction {
                baker: content.baker.clone(),
                protocol_req_id: req_id,
                request,
            });
        }
        Action::BakerBlockBakerPreapplySuccess(content) => {
            store.dispatch(BakerBlockBakerComputeProofOfWorkInitAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerComputeProofOfWorkInit(content) => {
            let header = match store.state.get().bakers.get(&content.baker) {
                Some(v) => match &v.block_baker {
                    BakerBlockBakerState::PreapplySuccess { header, .. } => header,
                    _ => return,
                },
                None => return,
            };
            let constants = match store.state.get().current_head.constants() {
                Some(v) => v,
                None => return,
            };
            let req_id = store.service.baker().compute_proof_of_work(
                content.baker.clone(),
                header.clone(),
                constants.proof_of_work_threshold,
            );

            store.dispatch(BakerBlockBakerComputeProofOfWorkPendingAction {
                baker: content.baker.clone(),
                req_id,
            });
        }
        Action::BakerBlockBakerComputeProofOfWorkSuccess(content) => {
            store.dispatch(BakerBlockBakerSignInitAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerSignInit(content) => {
            let header = match store.state.get().bakers.get(&content.baker) {
                Some(v) => match &v.block_baker {
                    BakerBlockBakerState::ComputeProofOfWorkSuccess { header, .. } => header,
                    _ => return,
                },
                None => return,
            };
            let chain_id = &store.state.get().config.chain_id;
            let req_id = store
                .service
                .baker()
                .block_sign(&content.baker, chain_id, header);

            store.dispatch(BakerBlockBakerSignPendingAction {
                baker: content.baker.clone(),
                req_id,
            });
        }
        Action::BakerBlockBakerSignSuccess(content) => {
            store.dispatch(BakerBlockBakerComputeOperationsPathsInitAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerComputeOperationsPathsInit(content) => {
            let baker_state = match store.state().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            let req = match &baker_state.block_baker {
                BakerBlockBakerState::SignSuccess { operations, .. } => {
                    match ComputePathRequest::try_from(operations) {
                        Ok(v) => v,
                        Err(_) => return,
                    }
                }
                _ => return,
            };
            let token = store
                .service
                .protocol_runner()
                .compute_operations_paths(req);
            store.dispatch(BakerBlockBakerComputeOperationsPathsPendingAction {
                baker: content.baker.clone(),
                protocol_req_id: token,
            });
        }
        Action::BakerBlockBakerComputeOperationsPathsSuccess(content) => {
            store.dispatch(BakerBlockBakerInjectInitAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockBakerInjectInit(content) => {
            let baker_state = match store.state().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            let header = match &baker_state.block_baker {
                BakerBlockBakerState::ComputeOperationsPathsSuccess { header, .. } => {
                    header.clone()
                }
                _ => return,
            };
            let block = BlockHeaderWithHash::new(header).unwrap();

            store.dispatch(BakerBlockBakerInjectPendingAction {
                baker: content.baker.clone(),
                block,
            });
        }
        Action::BakerBlockBakerInjectPending(content) => {
            let baker_state = match store.state().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            let (block, operations, operations_paths) = match &baker_state.block_baker {
                BakerBlockBakerState::InjectPending {
                    block,
                    operations,
                    operations_paths,
                    ..
                } => (block.clone(), operations.clone(), operations_paths.clone()),
                _ => return,
            };
            store.dispatch(BakerSeedNonceCommittedAction {
                baker: content.baker.clone(),
                level: block.header.level(),
                block_hash: block.hash.clone(),
            });
            let block_hash = block.hash.clone();
            let chain_id = store.state().config.chain_id.clone();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockHeaderPut(chain_id, block),
                requestor: StorageRequestor::BakerBlockBaker(content.baker.clone()),
            });

            for ((i, operations), operations_path) in
                operations.iter().enumerate().zip(operations_paths)
            {
                let ops = OperationsForBlocksMessage::new(
                    OperationsForBlock::new(block_hash.clone(), i as i8),
                    operations_path,
                    operations.clone(),
                );
                store.dispatch(StorageRequestCreateAction {
                    payload: StorageRequestPayload::BlockOperationsPut(ops),
                    requestor: StorageRequestor::BakerBlockBaker(content.baker.clone()),
                });
            }

            store.dispatch(BlockApplierEnqueueBlockAction {
                block_hash: block_hash.into(),
                injector_rpc_id: None,
            });
            store.dispatch(BakerBlockBakerInjectSuccessAction {
                baker: content.baker.clone(),
            });
        }
        Action::ProtocolRunnerResponse(content) => match &content.result {
            ProtocolRunnerResult::PreapplyBlock((token, result)) => {
                let bakers_iter = store.state().bakers.iter();
                let bakers_iter = bakers_iter.filter_map(|(baker, b)| match &b.block_baker {
                    BakerBlockBakerState::PreapplyPending {
                        protocol_req_id, ..
                    } => Some((baker, protocol_req_id)),
                    _ => None,
                });
                let baker = match bakers_iter.clone().find(|(_, req_id)| *req_id == token) {
                    Some((baker, req_id)) => {
                        slog::debug!(&store.state().log, "Baker block preapply response received";
                            "baker" => baker.to_base58_check(),
                            "req_id" => format!("{:?}", req_id),
                            "response" => format!("{:?}", result));
                        baker.clone()
                    }
                    None => {
                        let requests = bakers_iter.collect::<Vec<_>>();
                        slog::debug!(&store.state().log, "Unexpected block preapply response received";
                            "baker_pening_requests" => format!("{:?}", requests),
                            "req_id" => format!("{:?}", token),
                            "response" => format!("{:?}", result));
                        return;
                    }
                };

                match result {
                    Ok(response) => {
                        store.dispatch(BakerBlockBakerPreapplySuccessAction {
                            baker,
                            response: response.clone(),
                        });
                    }
                    Err(_) => {
                        // TODO(zura)
                        return;
                    }
                }
            }
            ProtocolRunnerResult::ComputeOperationsPaths((token, result)) => {
                let mut bakers_iter = store.state().bakers.iter();
                let baker = match bakers_iter
                    .find(|(_, b)| match &b.block_baker {
                        BakerBlockBakerState::ComputeOperationsPathsPending {
                            protocol_req_id,
                            ..
                        } => protocol_req_id == token,
                        _ => false,
                    })
                    .map(|(baker, _)| baker.clone())
                {
                    Some(v) => v,
                    None => return,
                };

                match result {
                    Ok(resp) => {
                        store.dispatch(BakerBlockBakerComputeOperationsPathsSuccessAction {
                            baker,
                            operations_paths: resp.operations_hashes_path.clone(),
                        });
                    }
                    Err(_) => {
                        todo!();
                    }
                }
            }
            _ => return,
        },
        _ => {}
    }
}
