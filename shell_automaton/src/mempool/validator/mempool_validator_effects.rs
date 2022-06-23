// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_api::ffi::{
    BeginConstructionRequest, ClassifiedOperation, Errored, OperationClassification,
    ValidateOperationRequest, ValidateOperationResult, Validated,
};

use crate::current_head::CurrentHeadState;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    MempoolValidatorPendingAction, MempoolValidatorReadyAction,
    MempoolValidatorReclassifyOperationAction, MempoolValidatorSuccessAction,
    MempoolValidatorValidatePendingAction, MempoolValidatorValidateResult,
    MempoolValidatorValidateSuccessAction,
};

pub fn mempool_validator_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    match &action.action {
        Action::MempoolValidatorInit(_) => {
            let chain_id = store.state().config.chain_id.clone();
            let req = match &store.state().current_head {
                CurrentHeadState::Rehydrated { head, .. } => BeginConstructionRequest {
                    chain_id,
                    predecessor: (*head.header).clone(),
                    predecessor_hash: head.hash.clone(),
                    protocol_data: None,
                },
                _ => return,
            };
            store.service().prevalidator().begin_construction(req);

            store.dispatch(MempoolValidatorPendingAction {});
        }
        Action::MempoolValidatorSuccess(_) => {
            store.dispatch(MempoolValidatorReadyAction {});
        }
        Action::MempoolValidatorValidateInit(content) => {
            let prevalidator = match &store.state().mempool.validator.prevalidator() {
                Some(v) => (*v).clone(),
                None => return,
            };
            let validate_req = ValidateOperationRequest {
                prevalidator,
                operation_hash: content.op_hash.clone(),
                operation: content.op_content.clone(),
                include_operation_data_json: true,
            };
            store
                .service()
                .prevalidator()
                .validate_operation(validate_req);
            store.dispatch(MempoolValidatorValidatePendingAction {});
        }
        Action::ProtocolRunnerResponse(resp) => match &resp.result {
            ProtocolRunnerResult::BeginConstruction((_, Err(err))) => {
                slog::warn!(&store.state().log, "Prevalidator error: {}", err);
            }
            ProtocolRunnerResult::ValidateOperation((_, Err(err))) => {
                slog::warn!(&store.state().log, "Prevalidator validation error: {}", err);
            }
            ProtocolRunnerResult::BeginConstruction((_, Ok(res))) => {
                store.dispatch(MempoolValidatorSuccessAction {
                    prevalidator: res.clone(),
                });
            }
            ProtocolRunnerResult::ValidateOperation((_, Ok(res))) => {
                if !store
                    .state()
                    .mempool
                    .validator
                    .prevalidator_matches(&res.prevalidator)
                {
                    slog::debug!(&store.state().log, "Got stale operation validation result";
                        "validated_for" => res.prevalidator.predecessor.to_base58_check(),
                        "current_mempool_head" => format!("{:?}", store.state().current_head.hash()));
                    return;
                }
                let result = match &res.result {
                    ValidateOperationResult::Unparseable => {
                        MempoolValidatorValidateResult::Unparseable(res.operation_hash.clone())
                    }
                    ValidateOperationResult::Classified(ClassifiedOperation {
                        classification,
                        operation_data_json,
                        is_endorsement,
                    }) => {
                        let is_endorsement = *is_endorsement;
                        // NOTE: should not happen right now, because we include the json data
                        // when validating an operation. In the future this will be done during prefilter
                        let protocol_data_json = operation_data_json
                            .clone()
                            .unwrap_or_else(|| "null".to_string());
                        match classification {
                            OperationClassification::Applied => {
                                MempoolValidatorValidateResult::Applied(Validated {
                                    hash: res.operation_hash.clone(),
                                    protocol_data_json,
                                })
                            }
                            OperationClassification::Prechecked => {
                                MempoolValidatorValidateResult::Prechecked(Validated {
                                    hash: res.operation_hash.clone(),
                                    protocol_data_json,
                                })
                            }
                            OperationClassification::BranchDelayed(error_json) => {
                                MempoolValidatorValidateResult::BranchDelayed(Errored {
                                    hash: res.operation_hash.clone(),
                                    protocol_data_json,
                                    error_json: error_json.clone(),
                                    is_endorsement,
                                })
                            }
                            OperationClassification::BranchRefused(error_json) => {
                                MempoolValidatorValidateResult::BranchRefused(Errored {
                                    hash: res.operation_hash.clone(),
                                    protocol_data_json,
                                    error_json: error_json.clone(),
                                    is_endorsement,
                                })
                            }
                            OperationClassification::Refused(error_json) => {
                                MempoolValidatorValidateResult::Refused(Errored {
                                    hash: res.operation_hash.clone(),
                                    protocol_data_json,
                                    error_json: error_json.clone(),
                                    is_endorsement,
                                })
                            }
                            OperationClassification::Outdated(error_json) => {
                                MempoolValidatorValidateResult::Outdated(Errored {
                                    hash: res.operation_hash.clone(),
                                    protocol_data_json,
                                    error_json: error_json.clone(),
                                    is_endorsement,
                                })
                            }
                        }
                    }
                };
                let op_hash = res.operation_hash.clone();
                store.dispatch(MempoolValidatorValidateSuccessAction {
                    op_hash,
                    result,
                    protocol_preapply_start: res.validate_operation_started_at,
                    protocol_preapply_end: res.validate_operation_ended_at,
                });

                // An old operation was reclassified by the prechecker
                if let Some((reclassified_op_hash, classification)) = &res.to_reclassify {
                    store.dispatch(MempoolValidatorReclassifyOperationAction {
                        op_hash: reclassified_op_hash.clone(),
                        classification: classification.clone(),
                    });
                }
            }
            _ => (),
        },
        _ => {}
    }
}
