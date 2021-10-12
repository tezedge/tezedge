use redux_rs::{ActionWithId, Store};

use crate::peer::disconnection::PeerDisconnectAction;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::service::actors_service::ActorsMessageFrom;
use crate::service::{ActorsService, Service};
use crate::{action::Action, State};

pub fn actors_effects<S: Service>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) {
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok(msg) = store.service.actors().try_recv() {
                match msg {
                    ActorsMessageFrom::Shutdown => {
                        // TODO
                    }
                    ActorsMessageFrom::PeerStalled(peer_id) => {
                        // TODO: blacklist as well.
                        store.dispatch(
                            PeerDisconnectAction {
                                address: peer_id.address,
                            }
                            .into(),
                        );
                    }
                    ActorsMessageFrom::BlacklistPeer(peer_id, _) => {
                        // TODO: blacklist as well.
                        store.dispatch(
                            PeerDisconnectAction {
                                address: peer_id.address,
                            }
                            .into(),
                        );
                    }
                    ActorsMessageFrom::SendMessage(peer_id, message) => {
                        store.dispatch(
                            PeerMessageWriteInitAction {
                                address: peer_id.address,
                                message,
                            }
                            .into(),
                        );
                    }
                }
            }
        }
        _ => {}
    }
}
