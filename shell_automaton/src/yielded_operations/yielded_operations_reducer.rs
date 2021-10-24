use crate::{Action, ActionWithId, State};

use super::YieldedOperationCurrent;

pub fn yielded_operations_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::YieldedOperationsAdd(action) => {
            state.yielded_operations.add(action.operation.clone());
        }
        Action::YieldedOperationsExecuteNextInit(_) => {
            if matches!(
                &state.yielded_operations.current,
                YieldedOperationCurrent::Init(_)
            ) {
                return;
            }

            if let Some(next_op) = state.yielded_operations.pop_front() {
                state.yielded_operations.current = YieldedOperationCurrent::Init(next_op);
            }
        }
        Action::YieldedOperationsExecuteNextSuccess(_) => {
            let current = &mut state.yielded_operations.current;

            if matches!(current, YieldedOperationCurrent::Init(_)) {
                *current = YieldedOperationCurrent::Success;
            }
        }
        _ => {}
    }
}
