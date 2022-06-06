// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::OperationHash;
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::operation::Operation;

use crate::baker::LockedPayload;
use crate::mempool::OperationKind;
use crate::{Action, ActionWithMeta, State};

use super::BakerBlockEndorserState;

fn should_set_locked_payload(state: &State) -> bool {
    let head_block = match state.current_head.get() {
        Some(v) => v,
        None => return false,
    };
    let head_locked_round = head_block.header.fitness().locked_round();

    head_locked_round.is_some() || state.mempool.prequorum.is_reached()
}

fn set_locked_payload(state: &mut State, baker_key: &SignaturePublicKey) -> Option<()> {
    if !should_set_locked_payload(state) {
        return None;
    }

    let baker = state.bakers.get_mut(baker_key)?;
    let head = &state.current_head;
    let head_block = head.get()?;
    let head_locked_round = head_block.header.fitness().locked_round();
    let new_locked_round = head_locked_round.or(head.round())?;
    let locked_round = baker.locked_payload.as_ref().map(|v| v.round());

    if locked_round.map_or(false, |locked_round| new_locked_round <= locked_round) {
        return None;
    }

    let mut operations = head.operations()?.clone();
    if !operations[0]
        .iter()
        .any(|op| OperationKind::from_operation_content_raw(op.data().as_ref()).is_preendorsement())
    {
        operations[0].extend(state.mempool.validated_operations.collect_preendorsements());
    }
    baker.locked_payload = Some(LockedPayload {
        block: head_block.clone(),
        round: new_locked_round,
        payload_hash: head.payload_hash()?.clone(),
        payload_round: head.payload_round()?,
        pred_header: (*head.get_pred()?.header).clone(),
        pred_block_metadata_hash: head.pred_block_metadata_hash()?.clone(),
        pred_ops_metadata_hash: head.pred_ops_metadata_hash()?.clone(),
        operations,
    });
    Some(())
}

pub fn baker_block_endorser_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            let head = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };
            for (_, baker) in state.bakers.iter_mut() {
                baker.block_endorser = BakerBlockEndorserState::Idle {
                    time: action.time_as_nanos(),
                };
                if baker
                    .locked_payload
                    .as_ref()
                    .map_or(false, |l| l.level() < head.header.level())
                {
                    baker.locked_payload = None;
                }
            }
            if should_set_locked_payload(state) {
                let baker_keys = state
                    .bakers
                    .iter()
                    .map(|(key, _)| key.clone())
                    .collect::<Vec<_>>();
                for baker in baker_keys {
                    set_locked_payload(state, &baker);
                }
            }
        }
        Action::BakerBlockEndorserRightsGetPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                baker.block_endorser = BakerBlockEndorserState::RightsGetPending {
                    time: action.time_as_nanos(),
                };
            }
        }
        Action::BakerBlockEndorserRightsNoRights(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let time = action.time_as_nanos();
                baker.block_endorser = BakerBlockEndorserState::NoRights { time };
            }
        }
        Action::BakerBlockEndorserRightsGetSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                baker.block_endorser = BakerBlockEndorserState::RightsGetSuccess {
                    time: action.time_as_nanos(),
                    first_slot: content.first_slot,
                };
            }
        }
        Action::BakerBlockEndorserPayloadOutdated(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::PayloadOutdated {
                    time: action.time_as_nanos(),
                    first_slot,
                };
            }
        }
        Action::BakerBlockEndorserPayloadLocked(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::PayloadLocked {
                    time: action.time_as_nanos(),
                    first_slot,
                };
            }
        }
        Action::BakerBlockEndorserPayloadUnlockedAsPreQuorumReached(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::PayloadUnlockedAsPreQuorumReached {
                    time: action.time_as_nanos(),
                    first_slot,
                };
            }
        }
        Action::BakerBlockEndorserPreendorse(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::Preendorse {
                    time: action.time_as_nanos(),
                    first_slot,
                };
            }
        }
        Action::BakerBlockEndorserPreendorsementSignPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::PreendorsementSignPending {
                    time: action.time_as_nanos(),
                    req_id: content.req_id,
                    first_slot,
                    operation: content.operation.clone(),
                };
            }
        }
        Action::BakerBlockEndorserPreendorsementSignSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_endorser {
                    BakerBlockEndorserState::PreendorsementSignPending {
                        first_slot,
                        operation,
                        ..
                    } => {
                        baker.block_endorser = BakerBlockEndorserState::PreendorsementSignSuccess {
                            time: action.time_as_nanos(),
                            first_slot: *first_slot,
                            operation: operation.clone(),
                            signature: content.signature.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockEndorserPreendorsementInjectPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_endorser {
                    BakerBlockEndorserState::PreendorsementSignSuccess {
                        first_slot,
                        operation,
                        signature,
                        ..
                    } => {
                        let signed_operation_bytes = operation
                            .forged()
                            .into_iter()
                            .chain(&signature.0)
                            .cloned()
                            .collect::<Vec<_>>();

                        let op = Operation::new(
                            operation.branch().clone(),
                            signed_operation_bytes.clone().into(),
                        );
                        let hash: OperationHash = op.message_typed_hash().unwrap();

                        baker.block_endorser =
                            BakerBlockEndorserState::PreendorsementInjectPending {
                                time: action.time_as_nanos(),
                                first_slot: *first_slot,
                                operation_hash: hash,
                                operation: operation.clone(),
                                signature: signature.clone(),
                                signed_operation_bytes,
                            };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockEndorserPreendorsementInjectSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_endorser {
                    BakerBlockEndorserState::PreendorsementInjectPending {
                        first_slot,
                        operation_hash,
                        operation,
                        signature,
                        signed_operation_bytes,
                        ..
                    } => {
                        baker.block_endorser =
                            BakerBlockEndorserState::PreendorsementInjectSuccess {
                                time: action.time_as_nanos(),
                                first_slot: *first_slot,
                                operation_hash: operation_hash.clone(),
                                operation: operation.clone(),
                                signature: signature.clone(),
                                signed_operation_bytes: signed_operation_bytes.clone(),
                            };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockEndorserPrequorumPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::PreQuorumPending {
                    time: action.time_as_nanos(),
                    first_slot,
                };
            }
        }
        Action::BakerBlockEndorserPrequorumSuccess(content) => {
            set_locked_payload(state, &content.baker);
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::PreQuorumSuccess {
                    time: action.time_as_nanos(),
                    first_slot,
                };
            }
        }
        Action::BakerBlockEndorserEndorse(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::Endorse {
                    time: action.time_as_nanos(),
                    first_slot,
                };
            }
        }
        Action::BakerBlockEndorserEndorsementSignPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let first_slot = match baker.block_endorser.first_slot() {
                    Some(v) => v,
                    None => return,
                };
                baker.block_endorser = BakerBlockEndorserState::EndorsementSignPending {
                    time: action.time_as_nanos(),
                    req_id: content.req_id,
                    first_slot,
                    operation: content.operation.clone(),
                };
            }
        }
        Action::BakerBlockEndorserEndorsementSignSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_endorser {
                    BakerBlockEndorserState::EndorsementSignPending {
                        first_slot,
                        operation,
                        ..
                    } => {
                        baker.block_endorser = BakerBlockEndorserState::EndorsementSignSuccess {
                            time: action.time_as_nanos(),
                            first_slot: *first_slot,
                            operation: operation.clone(),
                            signature: content.signature.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockEndorserEndorsementInjectPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_endorser {
                    BakerBlockEndorserState::EndorsementSignSuccess {
                        first_slot,
                        operation,
                        signature,
                        ..
                    } => {
                        let signed_operation_bytes = operation
                            .forged()
                            .into_iter()
                            .chain(&signature.0)
                            .cloned()
                            .collect::<Vec<_>>();

                        let op = Operation::new(
                            operation.branch().clone(),
                            signed_operation_bytes.clone().into(),
                        );
                        let hash: OperationHash = op.message_typed_hash().unwrap();

                        baker.block_endorser = BakerBlockEndorserState::EndorsementInjectPending {
                            time: action.time_as_nanos(),
                            first_slot: *first_slot,
                            operation_hash: hash,
                            operation: operation.clone(),
                            signature: signature.clone(),
                            signed_operation_bytes,
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockEndorserEndorsementInjectSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_endorser {
                    BakerBlockEndorserState::EndorsementInjectPending {
                        first_slot,
                        operation_hash,
                        operation,
                        signature,
                        signed_operation_bytes,
                        ..
                    } => {
                        baker.block_endorser = BakerBlockEndorserState::EndorsementInjectSuccess {
                            time: action.time_as_nanos(),
                            first_slot: *first_slot,
                            operation_hash: operation_hash.clone(),
                            operation: operation.clone(),
                            signature: signature.clone(),
                            signed_operation_bytes: signed_operation_bytes.clone(),
                        };
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}
