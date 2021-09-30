use redux_rs::{ActionWithId, Store};

use crate::peer::connection::outgoing::PeerConnectionOutgoingRandomInitAction;
use crate::service::Service;
use crate::{action::Action, State};

pub fn peers_add_multi_effects<S: Service>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) {
    match &action.action {
        Action::PeersAddMulti(_) => {
            store.dispatch(PeerConnectionOutgoingRandomInitAction {}.into());
        }
        _ => {}
    }
}
