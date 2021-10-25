use crate::{Action, ActionWithId, State};

use super::PausedLoopCurrent;

pub fn paused_loops_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PausedLoopsAdd(action) => {
            state.paused_loops.add(action.data.clone());
        }
        Action::PausedLoopsResumeNextInit(_) => {
            if matches!(&state.paused_loops.current, PausedLoopCurrent::Init(_)) {
                return;
            }

            if let Some(next_loop) = state.paused_loops.pop_front() {
                state.paused_loops.current = PausedLoopCurrent::Init(next_loop);
            }
        }
        Action::PausedLoopsResumeNextSuccess(_) => {
            let current = &mut state.paused_loops.current;

            if matches!(current, PausedLoopCurrent::Init(_)) {
                *current = PausedLoopCurrent::Success;
            }
        }
        _ => {}
    }
}
