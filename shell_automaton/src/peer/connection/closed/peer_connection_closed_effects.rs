use redux_rs::{ActionWithId, Store};

use crate::peers::graylist::PeersGraylistIpAddAction;
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
                PeersGraylistIpAddAction {
                    ip: action.address.ip(),
                }
                .into(),
            );
        }
        _ => {}
    }
}
