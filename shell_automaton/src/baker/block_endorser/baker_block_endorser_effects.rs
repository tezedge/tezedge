// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::encoding::operation::Operation;
use tezos_messages::protocol::proto_012::operation::{
    InlinedEndorsementMempoolContents, InlinedEndorsementMempoolContentsEndorsementVariant,
    InlinedPreendorsementContents, InlinedPreendorsementVariant,
};

use crate::mempool::MempoolOperationInjectAction;
use crate::prechecker::PrecheckerResultKind;
use crate::rights::rights_actions::RightsGetAction;
use crate::rights::RightsKey;
use crate::service::BakerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    BakerBlockEndorserEndorseAction, BakerBlockEndorserEndorsementInjectPendingAction,
    BakerBlockEndorserEndorsementInjectSuccessAction,
    BakerBlockEndorserEndorsementSignPendingAction, BakerBlockEndorserEndorsementSignSuccessAction,
    BakerBlockEndorserPreendorseAction, BakerBlockEndorserPreendorsementInjectPendingAction,
    BakerBlockEndorserPreendorsementInjectSuccessAction,
    BakerBlockEndorserPreendorsementSignPendingAction,
    BakerBlockEndorserPreendorsementSignSuccessAction, BakerBlockEndorserPrequorumPendingAction,
    BakerBlockEndorserPrequorumSuccessAction, BakerBlockEndorserRightsGetInitAction,
    BakerBlockEndorserRightsGetPendingAction, BakerBlockEndorserRightsGetSuccessAction,
    BakerBlockEndorserRightsNoRightsAction, BakerBlockEndorserState, EndorsementWithForgedBytes,
    PreendorsementWithForgedBytes,
};

pub fn baker_block_endorser_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            store.dispatch(BakerBlockEndorserRightsGetInitAction {});
        }
        Action::BakerBlockEndorserRightsGetInit(_) => {
            let current_head_hash = match store.state().current_head.get() {
                Some(v) => v.hash.clone(),
                None => return,
            };

            let bakers = store.state().baker_keys_iter().cloned().collect::<Vec<_>>();
            for baker in bakers {
                store.dispatch(BakerBlockEndorserRightsGetPendingAction { baker });
            }

            store.dispatch(RightsGetAction {
                key: RightsKey::endorsing(current_head_hash, None),
            });
        }
        Action::RightsEndorsingReady(content) => {
            let block = match store.state().current_head.get() {
                Some(v) => v,
                None => return,
            };
            let is_level_eq = content
                .key
                .level()
                .map_or(true, |level| level == block.header.level());
            if content.key.block() != &block.hash || !is_level_eq {
                return;
            }

            let level = block.header.level();
            let rights = store.state().rights.cache.endorsing.get(&level);
            let rights = match rights {
                Some((_, rights)) => rights,
                None => return,
            };
            let bakers_slots = store
                .state()
                .baker_keys_iter()
                .cloned()
                .map(|baker| {
                    let baker_key = baker.clone();
                    rights
                        .delegates
                        .get(&baker)
                        .map(|(first_slot, power)| (baker, *first_slot, *power))
                        .ok_or(baker_key)
                })
                .collect::<Vec<_>>();
            for baker_slots in bakers_slots {
                match baker_slots {
                    Ok((baker, first_slot, endorsing_power)) => {
                        store.dispatch(BakerBlockEndorserRightsGetSuccessAction {
                            baker,
                            first_slot,
                            endorsing_power,
                        })
                    }
                    Err(baker) => store.dispatch(BakerBlockEndorserRightsNoRightsAction { baker }),
                };
            }
        }
        Action::BakerBlockEndorserRightsGetSuccess(content) => {
            store.dispatch(BakerBlockEndorserPreendorseAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockEndorserPreendorse(content) => {
            let pred_hash = match store.state().current_head.get_pred() {
                Some(v) => v.hash.clone(),
                None => return,
            };
            let block_level = match store.state().current_head.level() {
                Some(v) => v,
                None => return,
            };
            let block_round = match store.state().current_head.round() {
                Some(v) => v,
                None => return,
            };
            let block_payload_hash = match store.state().current_head.payload_hash() {
                Some(v) => v.clone(),
                None => return,
            };
            let baker = match store.state().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            let first_slot = match baker.block_endorser.first_slot() {
                Some(v) => v,
                None => return,
            };

            let preendorsement = InlinedPreendorsementVariant {
                slot: first_slot,
                level: block_level,
                round: block_round,
                block_payload_hash,
            };
            let preendorsement = InlinedPreendorsementContents::Preendorsement(preendorsement);
            store.dispatch(BakerBlockEndorserPreendorsementSignPendingAction {
                baker: content.baker.clone(),
                operation: PreendorsementWithForgedBytes::new(pred_hash, preendorsement).unwrap(),
            });
        }
        Action::BakerBlockEndorserPreendorsementSignPending(content) => {
            let chain_id = &store.state.get().config.chain_id;
            let signature = store
                .service
                .baker()
                .preendrosement_sign(&content.baker, chain_id, &content.operation)
                .unwrap();

            store.dispatch(BakerBlockEndorserPreendorsementSignSuccessAction {
                baker: content.baker.clone(),
                signature,
            });
        }
        Action::BakerBlockEndorserPreendorsementSignSuccess(content) => {
            store.dispatch(BakerBlockEndorserPreendorsementInjectPendingAction {
                baker: content.baker.clone(),
            });
            let baker = match store.state().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            match &baker.block_endorser {
                BakerBlockEndorserState::PreendorsementInjectPending {
                    operation_hash,
                    operation,
                    signed_operation_bytes,
                    ..
                } => {
                    let hash = operation_hash.clone();
                    let operation = Operation::new(
                        operation.branch().clone(),
                        signed_operation_bytes.clone().into(),
                    );
                    store.dispatch(MempoolOperationInjectAction {
                        hash,
                        operation,
                        rpc_id: None,
                        injected_timestamp: action.time_as_nanos(),
                    });
                }
                _ => {}
            };
        }
        Action::MempoolValidatorValidateSuccess(content) => {
            for (baker, baker_state) in store.state().bakers.iter() {
                // TODO(zura): should be impossible but check if validation
                // result is indeed applied.
                match &baker_state.block_endorser {
                    BakerBlockEndorserState::PreendorsementInjectPending {
                        operation_hash, ..
                    } => {
                        if operation_hash != &content.op_hash {
                            continue;
                        }
                        let baker = baker.clone();
                        store.dispatch(BakerBlockEndorserPreendorsementInjectSuccessAction {
                            baker,
                        });
                        return;
                    }
                    BakerBlockEndorserState::EndorsementInjectPending {
                        operation_hash, ..
                    } => {
                        if operation_hash != &content.op_hash {
                            continue;
                        }
                        let baker = baker.clone();
                        store.dispatch(BakerBlockEndorserEndorsementInjectSuccessAction { baker });
                        return;
                    }
                    _ => {}
                }
            }
        }
        Action::PrecheckerOperationValidated(content) => {
            let result = match store.state().prechecker.result(&content.hash) {
                Some(v) => v,
                None => return,
            };
            for (baker, baker_state) in store.state().bakers.iter() {
                // TODO(zura): should be impossible but check if validation
                // result is indeed applied.
                match &baker_state.block_endorser {
                    BakerBlockEndorserState::PreendorsementInjectPending {
                        operation_hash, ..
                    } => {
                        if &content.hash != operation_hash {
                            continue;
                        }
                        let baker = baker.clone();
                        match result.kind() {
                            PrecheckerResultKind::Applied { .. } => {
                                store.dispatch(
                                    BakerBlockEndorserPreendorsementInjectSuccessAction { baker },
                                );
                            }
                            _ => {
                                // TODO(zura)
                            }
                        }

                        return;
                    }
                    BakerBlockEndorserState::EndorsementInjectPending {
                        operation_hash, ..
                    } => {
                        if &content.hash != operation_hash {
                            continue;
                        }
                        let baker = baker.clone();
                        match result.kind() {
                            PrecheckerResultKind::Applied { .. } => {
                                store.dispatch(BakerBlockEndorserEndorsementInjectSuccessAction {
                                    baker,
                                });
                            }
                            _ => {
                                // TODO(zura)
                            }
                        }

                        return;
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockEndorserPreendorsementInjectSuccess(content) => {
            store.dispatch(BakerBlockEndorserPrequorumPendingAction {
                baker: content.baker.clone(),
            });
        }
        Action::MempoolPrequorumReached(_) => {
            let bakers = store.state().baker_keys_iter().cloned().collect::<Vec<_>>();
            for baker in bakers {
                store.dispatch(BakerBlockEndorserPrequorumSuccessAction { baker });
            }
        }
        Action::BakerBlockEndorserPrequorumSuccess(content) => {
            store.dispatch(BakerBlockEndorserEndorseAction {
                baker: content.baker.clone(),
            });
        }
        Action::BakerBlockEndorserEndorse(content) => {
            let pred_hash = match store.state().current_head.get_pred() {
                Some(v) => v.hash.clone(),
                None => return,
            };
            let block_level = match store.state().current_head.level() {
                Some(v) => v,
                None => return,
            };
            let block_round = match store.state().current_head.round() {
                Some(v) => v,
                None => return,
            };
            let block_payload_hash = match store.state().current_head.payload_hash() {
                Some(v) => v.clone(),
                None => return,
            };
            let baker = match store.state().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            let first_slot = match baker.block_endorser.first_slot() {
                Some(v) => v,
                None => return,
            };

            let endorsement = InlinedEndorsementMempoolContentsEndorsementVariant {
                slot: first_slot,
                level: block_level,
                round: block_round,
                block_payload_hash,
            };
            let endorsement = InlinedEndorsementMempoolContents::Endorsement(endorsement);

            store.dispatch(BakerBlockEndorserEndorsementSignPendingAction {
                baker: content.baker.clone(),
                operation: EndorsementWithForgedBytes::new(pred_hash, endorsement).unwrap(),
            });
        }
        Action::BakerBlockEndorserEndorsementSignPending(content) => {
            let chain_id = &store.state.get().config.chain_id;
            let signature = store
                .service
                .baker()
                .endrosement_sign(&content.baker, chain_id, &content.operation)
                .unwrap();

            store.dispatch(BakerBlockEndorserEndorsementSignSuccessAction {
                baker: content.baker.clone(),
                signature,
            });
        }
        Action::BakerBlockEndorserEndorsementSignSuccess(content) => {
            store.dispatch(BakerBlockEndorserEndorsementInjectPendingAction {
                baker: content.baker.clone(),
            });
            let baker = match store.state().bakers.get(&content.baker) {
                Some(v) => v,
                None => return,
            };
            match &baker.block_endorser {
                BakerBlockEndorserState::EndorsementInjectPending {
                    operation_hash,
                    operation,
                    signed_operation_bytes,
                    ..
                } => {
                    let hash = operation_hash.clone();
                    let operation = Operation::new(
                        operation.branch().clone(),
                        signed_operation_bytes.clone().into(),
                    );
                    store.dispatch(MempoolOperationInjectAction {
                        hash,
                        operation,
                        rpc_id: None,
                        injected_timestamp: action.time_as_nanos(),
                    });
                }
                _ => {}
            };
        }

        _ => {}
    }
}
