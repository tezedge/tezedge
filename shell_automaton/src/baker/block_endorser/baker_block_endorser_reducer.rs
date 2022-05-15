// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::OperationHash;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::operation::Operation;

use crate::baker::LockedPayload;
use crate::{Action, ActionWithMeta, State};

use super::BakerBlockEndorserState;

pub fn baker_block_endorser_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            for (_, baker) in state.bakers.iter_mut() {
                baker.block_endorser = BakerBlockEndorserState::Idle {
                    time: action.time_as_nanos(),
                };
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
                        let op = operation.operation();
                        baker.locked_payload = Some(LockedPayload {
                            level: op.level,
                            round: op.round,
                            payload_hash: op.block_payload_hash.clone(),
                        });
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
