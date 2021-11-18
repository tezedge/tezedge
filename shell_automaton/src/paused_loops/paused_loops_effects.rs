// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::peer::{PeerTryReadLoopStartAction, PeerTryWriteLoopStartAction};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    PausedLoop, PausedLoopCurrent, PausedLoopsResumeNextInitAction,
    PausedLoopsResumeNextSuccessAction,
};

pub fn paused_loops_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::PausedLoopsResumeAll(_) => {
            for _ in 0..state.paused_loops.len() {
                store.dispatch(PausedLoopsResumeNextInitAction {});
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
                    store.dispatch(PeerTryWriteLoopStartAction { address });
                }
                PausedLoop::PeerTryRead { peer_address } => {
                    let address = *peer_address;
                    store.dispatch(PeerTryReadLoopStartAction { address });
                }
            }

            store.dispatch(PausedLoopsResumeNextSuccessAction {});
        }
        _ => {}
    }
}
