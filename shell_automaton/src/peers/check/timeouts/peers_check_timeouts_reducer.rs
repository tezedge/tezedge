use crate::peers::check::timeouts::PeersCheckTimeoutsState;
use crate::{Action, ActionWithId, State};

pub fn peers_check_timeouts_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeersCheckTimeoutsInit(_) => {
            if matches!(
                &state.peers.check_timeouts,
                PeersCheckTimeoutsState::Idle { .. }
            ) {
                state.peers.check_timeouts = PeersCheckTimeoutsState::Init {
                    time: action.time_as_nanos(),
                };
            }
        }
        Action::PeersCheckTimeoutsSuccess(action_content) => {
            let timeouts = action_content.timeouts.clone();

            state.peers.check_timeouts = PeersCheckTimeoutsState::Success {
                time: action.time_as_nanos(),
                timeouts,
            };
        }
        Action::PeersCheckTimeoutsCleanup(_) => {
            state.peers.check_timeouts = PeersCheckTimeoutsState::Idle {
                time: action.time_as_nanos(),
            };
        }
        _ => {}
    }
}
