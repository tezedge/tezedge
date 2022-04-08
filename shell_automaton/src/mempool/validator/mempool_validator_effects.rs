// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};

use crate::current_head::CurrentHeadState;
use crate::service::protocol_runner_service::ProtocolRunnerResult;
use crate::service::ProtocolRunnerService;
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    MempoolValidatorPendingAction, MempoolValidatorReadyAction, MempoolValidatorSuccessAction,
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
                CurrentHeadState::Rehydrated {
                    head,
                    block_metadata_hash,
                    ops_metadata_hash,
                    ..
                } => BeginConstructionRequest {
                    chain_id,
                    predecessor: (*head.header).clone(),
                    predecessor_hash: head.hash.clone(),
                    protocol_data: None,
                    predecessor_block_metadata_hash: block_metadata_hash.clone(),
                    predecessor_ops_metadata_hash: ops_metadata_hash.clone(),
                },
                _ => return,
            };
            store
                .service()
                .prevalidator()
                .begin_construction_for_prevalidation(req);

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
                operation: content.op_content.clone(),
            };
            store
                .service()
                .prevalidator()
                .validate_operation_for_prevalidation(validate_req);
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
                        "current_mempool_head" => format!("{:?}", store.state().current_head.get_hash()));
                    return;
                }
                let (op_hash, result) = {
                    if let Some(data) = res.result.applied.first() {
                        (
                            data.hash.clone(),
                            MempoolValidatorValidateResult::Applied(data.clone()),
                        )
                    } else if let Some(data) = res.result.refused.first() {
                        (
                            data.hash.clone(),
                            MempoolValidatorValidateResult::Refused(data.clone()),
                        )
                    } else if let Some(data) = res.result.branch_refused.first() {
                        (
                            data.hash.clone(),
                            MempoolValidatorValidateResult::BranchRefused(data.clone()),
                        )
                    } else if let Some(data) = res.result.branch_delayed.first() {
                        (
                            data.hash.clone(),
                            MempoolValidatorValidateResult::BranchDelayed(data.clone()),
                        )
                    } else if let Some(data) = res.result.outdated.first() {
                        (
                            data.hash.clone(),
                            MempoolValidatorValidateResult::Outdated(data.clone()),
                        )
                    } else {
                        return;
                    }
                };
                store.dispatch(MempoolValidatorValidateSuccessAction {
                    op_hash,
                    result,
                    protocol_preapply_start: res.validate_operation_started_at,
                    protocol_preapply_end: res.validate_operation_ended_at,
                });
            }
            _ => (),
        },
        _ => {}
    }
}
