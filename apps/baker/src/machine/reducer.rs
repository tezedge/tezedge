// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use redux_rs::ActionWithMeta;
use tenderbake as tb;

use crate::services::EventWithTime;

use super::{actions::*, state::BakerState};

pub fn baker_reducer<S, A>(state: &mut S, action: &ActionWithMeta<A>)
where
    S: AsMut<Option<BakerState>>,
    A: AsRef<Option<BakerAction>>,
{
    match action.action.as_ref() {
        // not our action
        None => (),
        Some(baker_action) => if baker_action.is_event() {
            let now = tb::Timestamp {
                unix_epoch: Duration::from_nanos(action.time_as_nanos()),
            };

            let event = EventWithTime {
                action: baker_action.clone(),
                now,
            };
            let baker_state = state
                .as_mut()
                .take()
                .expect("baker state should not be empty outside of this reducer");
            let new_baker_state = baker_state.handle_event(event);
            *state.as_mut() = Some(new_baker_state);
        }
    }
}
