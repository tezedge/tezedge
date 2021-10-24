use redux_rs::Store;

use crate::peer::{PeerTryReadLoopStartAction, PeerTryWriteLoopStartAction};
use crate::{Action, ActionWithId, Service, State};

use super::{
    YieldedOperation, YieldedOperationCurrent, YieldedOperationsExecuteNextInitAction,
    YieldedOperationsExecuteNextSuccessAction,
};

pub fn yielded_operations_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    let state = store.state.get();

    match &action.action {
        Action::YieldedOperationsExecuteAll(_) => {
            let operations_len = state.yielded_operations.len();

            for _ in 0..operations_len {
                store.dispatch(YieldedOperationsExecuteNextInitAction {}.into())
            }
        }
        Action::YieldedOperationsExecuteNextInit(_) => {
            let operation = match &state.yielded_operations.current {
                YieldedOperationCurrent::Init(op) => op,
                _ => return,
            };

            match operation {
                YieldedOperation::PeerTryWriteLoop { peer_address } => {
                    let address = *peer_address;
                    store.dispatch(PeerTryWriteLoopStartAction { address }.into());
                }
                YieldedOperation::PeerTryReadLoop { peer_address } => {
                    let address = *peer_address;
                    store.dispatch(PeerTryReadLoopStartAction { address }.into());
                }
            }

            store.dispatch(YieldedOperationsExecuteNextSuccessAction {}.into());
        }
        _ => {}
    }
}
