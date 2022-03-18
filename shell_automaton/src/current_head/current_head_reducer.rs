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
                head_pred: content.head_pred.clone(),
            };
        }
        Action::CurrentHeadRehydrated(_) => {
            let (head, head_pred) = match &state.current_head {
                CurrentHeadState::RehydrateSuccess {
                    head, head_pred, ..
                } => (head.clone(), head_pred.clone()),
                _ => return,
            };
            state.current_head = CurrentHeadState::Rehydrated { head, head_pred };
        }
        Action::CurrentHeadUpdate(content) => {
            let head_pred = state
                .current_head
                .get()
                .filter(|pred| &pred.hash == content.new_head.header.predecessor())
                .or_else(|| state.current_head.get_pred())
                .filter(|pred| &pred.hash == content.new_head.header.predecessor())
                .cloned();
            state.current_head = CurrentHeadState::Rehydrated {
                head: content.new_head.as_ref().clone(),
                head_pred,
            };
        }
        _ => {}
    }
}
