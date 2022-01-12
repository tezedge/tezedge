// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::ActionWithMeta;

use super::{action::Action, state::State};

pub fn reducer(state: &mut State, action: &ActionWithMeta<Action>) {
    match &action.action {
        Action::Bootstrapped(_) => {
            state.is_bootstrapped = true;
        }
        _ => {}
    }
}
