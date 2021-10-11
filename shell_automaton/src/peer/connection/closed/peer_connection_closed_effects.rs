use redux_rs::{ActionWithId, Store};

use crate::peer::disconnection::PeerDisconnectAction;
use crate::service::Service;
use crate::{Action, State};

pub fn peer_connection_closed_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerConnectionClosed(action) => {
            store.dispatch(
                PeerDisconnectAction {
                    address: action.address,
                }
                .into(),
            );
        }
        _ => {}
    }
}
