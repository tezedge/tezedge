// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::Action;

use super::{rights_cycle_eras_actions::*, CycleErasError, CycleErasQuery, CycleErasQueryState};

pub fn rights_cycle_eras_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    let eras_state = &mut state.rights.cycle_eras;
    match &action.action {
        Action::RightsCycleErasGet(RightsCycleErasGetAction {
            block_hash,
            block_header,
            protocol_hash,
        }) => {
            eras_state.insert(
                protocol_hash.clone(),
                CycleErasQuery {
                    block_hash: block_hash.clone(),
                    block_header: block_header.clone(),
                    state: CycleErasQueryState::PendingKV,
                },
            );
        }
        Action::RightsCycleErasKVSuccess(RightsCycleErasKVSuccessAction {
            protocol_hash,
            cycle_eras,
        }) => {
            if let Some(req_state) = eras_state.get_state_mut(protocol_hash) {
                slog::debug!(state.log, "Cycle eras from kv store: {cycle_eras:#?}");
                *req_state = CycleErasQueryState::Success(cycle_eras.clone())
            }
        }
        Action::RightsCycleErasKVError(RightsCycleErasKVErrorAction {
            protocol_hash,
            kv_store_error,
        }) => {
            if let Some(state) = eras_state.get_state_mut(protocol_hash) {
                *state = CycleErasQueryState::PendingContext(kv_store_error.clone());
            }
        }
        Action::RightsCycleErasContextRequested(RightsCycleErasContextRequestedAction {
            protocol_hash,
            token,
        }) => {
            if let Some(state) = eras_state.get_state_mut(protocol_hash) {
                if let CycleErasQueryState::PendingContext(kv_error) = state {
                    *state = CycleErasQueryState::ContextRequested(*token, kv_error.clone());
                }
            }
        }
        Action::RightsCycleErasContextSuccess(RightsCycleErasContextSuccessAction {
            protocol_hash,
            cycle_eras,
        }) => {
            if let Some(req_state) = eras_state.get_state_mut(protocol_hash) {
                slog::debug!(state.log, "Cycle eras from context: {cycle_eras:#?}");
                *req_state = CycleErasQueryState::Success(cycle_eras.clone())
            }
        }
        Action::RightsCycleErasContextError(RightsCycleErasContextErrorAction {
            protocol_hash,
            error,
        }) => {
            if let Some(state) = eras_state.get_state_mut(protocol_hash) {
                if let CycleErasQueryState::ContextRequested(_, kv_store_error) = state {
                    *state = CycleErasQueryState::Error(CycleErasError::new(
                        kv_store_error.clone(),
                        error.clone(),
                    ));
                }
            }
        }

        _ => (),
    }
}
