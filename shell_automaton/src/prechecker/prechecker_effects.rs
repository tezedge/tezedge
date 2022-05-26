// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::protocol::proto_012::FitnessRepr;

use crate::{
    mempool::MempoolOperationDecodedAction,
    rights::{
        rights_actions::{RightsEndorsingReadyAction, RightsGetAction},
        RightsKey,
    },
    Action, ActionWithMeta, Service, Store,
};

use super::{
    prechecker_actions::*, EndorsementBranch, PrecheckerOperation, PrecheckerOperationState,
};

pub fn prechecker_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let prechecker_state = &store.state.get().prechecker;
    let prechecker_state_operations = &prechecker_state.operations;
    match &action.action {
        Action::PrecheckerPrecheckOperation(action) => {
            match prechecker_state_operations.get(&action.hash) {
                Some(Ok(_)) => {
                    store.dispatch(PrecheckerDecodeOperationAction::from(&action.hash));
                }
                Some(Err(_)) => {
                    store.dispatch(PrecheckerErrorAction::from(&action.hash));
                    store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                }
                _ => {}
            }
        }
        Action::PrecheckerRevalidateOperation(action) => {
            match prechecker_state_operations.get(&action.hash) {
                Some(Ok(_)) => {
                    store.dispatch(PrecheckerValidateOperationAction::from(&action.hash));
                }
                Some(Err(_)) => {
                    store.dispatch(PrecheckerErrorAction::from(&action.hash));
                    store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                }
                _ => {}
            }
        }
        Action::PrecheckerDecodeOperation(action) => {
            match prechecker_state_operations.get(&action.hash) {
                Some(Ok(PrecheckerOperation {
                    state:
                        PrecheckerOperationState::Decoded {
                            operation_decoded_contents,
                        },
                    ..
                })) => {
                    let operation_decoded_contents = operation_decoded_contents.clone();
                    store.dispatch(MempoolOperationDecodedAction {
                        operation: action.hash.clone(),
                        operation_decoded_contents,
                    });
                    store.dispatch(PrecheckerCategorizeOperationAction::from(&action.hash));
                }
                Some(Err(_)) => {
                    store.dispatch(PrecheckerErrorAction::from(&action.hash));
                    store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                }
                _ => {}
            }
        }
        Action::PrecheckerCategorizeOperation(action) => {
            match prechecker_state_operations.get(&action.hash) {
                Some(Ok(op)) => match &op.state {
                    PrecheckerOperationState::TenderbakeConsensus { .. } => {
                        store.dispatch(PrecheckerValidateOperationAction::from(&action.hash));
                    }
                    PrecheckerOperationState::ProtocolNeeded => {
                        store.dispatch(PrecheckerProtocolNeededAction::from(&action.hash));
                        store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                    }
                    _ => {}
                },
                Some(Err(_)) => {
                    store.dispatch(PrecheckerErrorAction::from(&action.hash));
                    store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                }
                _ => {}
            }
        }
        Action::PrecheckerValidateOperation(action) => {
            match prechecker_state_operations.get(&action.hash) {
                Some(Ok(op)) => match &op.state {
                    state if state.is_result() => {
                        store.dispatch(PrecheckerOperationValidatedAction::from(&action.hash));
                        if !store
                            .dispatch(PrecheckerCacheDelayedOperationAction::from(&action.hash))
                        {
                            store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                        }
                    }
                    PrecheckerOperationState::ProtocolNeeded => {
                        store.dispatch(PrecheckerProtocolNeededAction::from(&action.hash));
                        store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                    }
                    PrecheckerOperationState::TenderbakePendingRights {
                        operation_decoded_contents,
                        consensus_contents,
                        ..
                    } => {
                        let current_block_hash = operation_decoded_contents.branch().clone();
                        let level = consensus_contents.level;
                        slog::debug!(
                            store.state().log,
                            "requesting rights for {level} using {current_block_hash}"
                        );
                        store.dispatch(RightsGetAction {
                            key: RightsKey::endorsing(current_block_hash, Some(level)),
                        });
                    }
                    _ => {}
                },
                Some(Err(_)) => {
                    store.dispatch(PrecheckerErrorAction::from(&action.hash));
                    store.dispatch(PrecheckerPruneOperationAction::from(&action.hash));
                }
                _ => {}
            }
        }
        Action::RightsEndorsingReady(RightsEndorsingReadyAction { key }) => {
            if let Some((current_block_hash, Some(level))) = key.endorsing_input() {
                for hash in prechecker_state_operations
                    .iter()
                    .filter_map(|(hash, op)| if matches!(&op, Ok(PrecheckerOperation { state: PrecheckerOperationState::TenderbakePendingRights {
                        operation_decoded_contents,
                        consensus_contents,
                        ..
                    }, ..}) if current_block_hash == operation_decoded_contents.branch() && &consensus_contents.level == level ) {
                        Some(hash)
                    } else {
                        None
                    }
                    )
                    .cloned()
                    .collect::<Vec<_>>()
                {
                    store.dispatch(PrecheckerValidateOperationAction { hash });
                }
            }
        }

        Action::PrecheckerCurrentHeadUpdate(PrecheckerCurrentHeadUpdateAction {
            protocol,
            payload_hash,
            head,
        }) => {
            if !store.state().is_bootstrapped() {
                return;
            }
            let level = head.header.level();
            let payload_hash = payload_hash.clone();
            let block_hash = head.hash.clone();
            let endorsement_branch = if let (Some(payload_hash), Ok(fitness)) =
                (payload_hash, FitnessRepr::try_from(head.header.fitness()))
            {
                let predecessor = head.header.predecessor().clone();
                Some(EndorsementBranch {
                    predecessor,
                    payload_hash,
                    level: fitness.level,
                    round: fitness.round,
                })
            } else {
                None
            };
            store.dispatch(PrecheckerCacheProtocolAction {
                proto: head.header.proto(),
                protocol_hash: protocol.clone(),
            });
            if !store.state.get().config.disable_endorsements_precheck {
                store.dispatch(PrecheckerStoreEndorsementBranchAction { endorsement_branch });
                store.dispatch(RightsGetAction {
                    key: RightsKey::endorsing(block_hash, Some(level + 1)),
                });
            }
        }

        _ => (),
    }
}
