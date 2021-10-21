use redux_rs::{ActionWithId, Store};

use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peers::graylist::PeersGraylistIpAddAction;
use crate::service::actors_service::ActorsMessageFrom;
use crate::service::{ActorsService, Service};
use crate::{Action, State};

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
                        store.dispatch(
                            PeersGraylistIpAddAction {
                                ip: peer_id.address.ip(),
                            }
                            .into(),
                        );
                    }
                    ActorsMessageFrom::BlacklistPeer(peer_id, _) => {
                        store.dispatch(
                            PeersGraylistIpAddAction {
                                ip: peer_id.address.ip(),
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
