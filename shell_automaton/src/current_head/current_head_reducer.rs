// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::CurrentHeadState;

pub fn current_head_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::CurrentHeadRehydrateInit(_) => {
            state.current_head = CurrentHeadState::RehydrateInit {
                time: action.time_as_nanos(),
            };
        }
        Action::CurrentHeadRehydratePending(content) => {
            state.current_head = CurrentHeadState::RehydratePending {
                time: action.time_as_nanos(),
                storage_req_id: content.storage_req_id,
            };
        }
        Action::CurrentHeadRehydrateError(content) => {
            state.current_head = CurrentHeadState::RehydrateError {
                time: action.time_as_nanos(),
                error: content.error.clone(),
            };
        }
        Action::CurrentHeadRehydrateSuccess(content) => {
            state.current_head = CurrentHeadState::RehydrateSuccess {
                time: action.time_as_nanos(),
                head: content.head.clone(),
            };
        }
        Action::CurrentHeadRehydrated(_) => {
            let head = match &state.current_head {
                CurrentHeadState::RehydrateSuccess { head, .. } => head.clone(),
                _ => return,
            };
            state.current_head = CurrentHeadState::Rehydrated { head };
        }
        Action::CurrentHeadUpdate(content) => {
            state.current_head = CurrentHeadState::Rehydrated {
                head: content.new_head.clone(),
            };
        }
        _ => {}
    }
}
