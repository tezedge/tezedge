// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use either::Either;
use redux_rs::ActionWithMeta;
use tenderbake as tb;

use crate::services::EventWithTime;

use super::{state::BakerState, actions::*};

pub fn baker_reducer<S, A>(state: &mut S, action: &ActionWithMeta<A>)
where
    S: AsMut<Option<BakerState>>,
    A: AsRef<Option<BakerAction>>,
{
    let now = tb::Timestamp {
        unix_epoch: Duration::from_nanos(action.time_as_nanos()),
    };
    match action.action.as_ref() {
        None => (),
        Some(baker_action) => {
            match baker_action.clone().into() {
                Either::Left(event) => {
                    let event = EventWithTime { event, now };
                    let baker_state = state.as_mut().take()
                        .expect("baker state should not be empty outside of this reducer");
                    let new_baker_state = baker_state.handle_event(event);
                    *state.as_mut() = Some(new_baker_state);
                },
                Either::Right(_) => (),
            }
        }
    }
}
