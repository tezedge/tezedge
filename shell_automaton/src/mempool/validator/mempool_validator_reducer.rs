// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::{MempoolValidatorState, MempoolValidatorValidateState};

pub fn mempool_validator_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::MempoolValidatorInit(_) => {
            state.mempool.validator = MempoolValidatorState::Init {
                time: action.time_as_nanos(),
            };
        }
        Action::MempoolValidatorPending(_) => {
            let block_hash = match state.current_head.get() {
                Some(v) => v.hash.clone(),
                None => return,
            };
            state.mempool.validator = MempoolValidatorState::Pending {
                time: action.time_as_nanos(),
                block_hash,
            };
        }
        Action::MempoolValidatorSuccess(content) => {
            match &state.mempool.validator {
                MempoolValidatorState::Pending {
                    time, block_hash, ..
                } => {
                    let dur = action.time_as_nanos() - time;
                    slog::info!(&state.log, "Constructed prevalidator";
                        "block_hash" => block_hash.to_base58_check(),
                        "duration" => format!("{}ms", dur / 1_000_000));
                }
                _ => {}
            }
            state.mempool.validator = MempoolValidatorState::Success {
                time: action.time_as_nanos(),
                prevalidator: content.prevalidator.clone(),
            };
        }
        Action::MempoolValidatorReady(_) => {
            let prevalidator = match state.mempool.validator.prevalidator() {
                Some(v) => v.clone(),
                None => return,
            };
            state.mempool.validator = MempoolValidatorState::Ready {
                time: action.time_as_nanos(),
                prevalidator,
                validate: MempoolValidatorValidateState::Idle {
                    time: action.time_as_nanos(),
                },
            };
        }
        Action::MempoolValidatorValidateInit(content) => {
            let validate = match &mut state.mempool.validator {
                MempoolValidatorState::Ready { validate, .. } => validate,
                _ => return,
            };
            *validate = MempoolValidatorValidateState::Init {
                time: action.time_as_nanos(),
                op_hash: content.op_hash.clone(),
                op_content: content.op_content.clone(),
            };
        }
        Action::MempoolValidatorValidatePending(_) => {
            let validate = match &mut state.mempool.validator {
                MempoolValidatorState::Ready { validate, .. } => validate,
                _ => return,
            };
            match validate {
                MempoolValidatorValidateState::Init {
                    op_hash,
                    op_content,
                    ..
                } => {
                    *validate = MempoolValidatorValidateState::Pending {
                        time: action.time_as_nanos(),
                        op_hash: op_hash.clone(),
                        op_content: op_content.clone(),
                    };
                }
                _ => return,
            }
        }
        Action::MempoolValidatorValidateSuccess(content) => {
            let validate = match &mut state.mempool.validator {
                MempoolValidatorState::Ready { validate, .. } => validate,
                _ => return,
            };
            match validate {
                MempoolValidatorValidateState::Pending { op_hash, .. } => {
                    *validate = MempoolValidatorValidateState::Success {
                        time: action.time_as_nanos(),
                        op_hash: op_hash.clone(),
                        result: content.result.clone(),
                        protocol_preapply_start: content.protocol_preapply_start,
                        protocol_preapply_end: content.protocol_preapply_end,
                    };
                }
                _ => return,
            }
        }
        _ => {}
    }
}
