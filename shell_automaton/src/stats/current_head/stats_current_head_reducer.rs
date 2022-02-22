// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::Action;

use super::stats_current_head_actions::*;

pub fn stats_current_head_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    match &action.action {
        Action::StatsCurrentHeadPrepareSend(StatsCurrentHeadPrepareSendAction {
            address,
            message,
        }) => {
            state
                .stats
                .current_head
                .pending_messages
                .insert(*address, message.clone());
        }
        Action::StatsCurrentHeadSent(StatsCurrentHeadSentAction { address })
        | Action::StatsCurrentHeadSentError(StatsCurrentHeadSentErrorAction { address }) => {
            state.stats.current_head.pending_messages.remove(address);
        }
        _ => (),
    }
}
