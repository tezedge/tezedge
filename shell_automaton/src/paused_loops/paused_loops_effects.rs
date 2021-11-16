// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use redux_rs::Store;

use crate::peer::{PeerTryReadLoopStartAction, PeerTryWriteLoopStartAction};
use crate::{Action, ActionWithId, Service, State};

use super::{
    PausedLoop, PausedLoopCurrent, PausedLoopsResumeNextInitAction,
    PausedLoopsResumeNextSuccessAction,
};

pub fn paused_loops_effects<S>(store: &mut Store<State, S, Action>, action: &ActionWithId<Action>)
where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::PausedLoopsResumeAll(_) => {
            for _ in 0..state.paused_loops.len() {
                store.dispatch(PausedLoopsResumeNextInitAction {}.into())
            }
        }
        Action::PausedLoopsResumeNextInit(_) => {
            let paused_loop = match &state.paused_loops.current {
                PausedLoopCurrent::Init(v) => v,
                _ => return,
            };

            match paused_loop {
                PausedLoop::PeerTryWrite { peer_address } => {
                    let address = *peer_address;
                    store.dispatch(PeerTryWriteLoopStartAction { address }.into());
                }
                PausedLoop::PeerTryRead { peer_address } => {
                    let address = *peer_address;
                    store.dispatch(PeerTryReadLoopStartAction { address }.into());
                }
            }

            store.dispatch(PausedLoopsResumeNextSuccessAction {}.into());
        }
        _ => {}
    }
}
