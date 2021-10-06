use redux_rs::{ActionWithId, Store};
use std::sync::Arc;

use tezos_messages::p2p::encoding::metadata::MetadataMessage;

use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peer::{PeerId, PeerStatus};
use crate::service::actors_service::{ActorsMessageFrom, ActorsMessageTo};
use crate::service::{ActorsService, Service};
use crate::{action::Action, State};

pub fn actors_effects<S: Service>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) {
    match &action.action {
        Action::PeerHandshakingFinish(action) => {
            let peer_handshaked = match store.state.get().peers.get(&action.address) {
                Some(peer) => match &peer.status {
                    PeerStatus::Handshaked(v) => v,
                    _ => return,
                },
                None => return,
            };
            store.service.actors().send(ActorsMessageTo::PeerHandshaked(
                Arc::new(PeerId {
                    address: action.address,
                    public_key_hash: peer_handshaked.public_key_hash.clone(),
                }),
                MetadataMessage::new(
                    peer_handshaked.disable_mempool,
                    peer_handshaked.private_node,
                ),
                Arc::new(peer_handshaked.version.clone()),
            ));
        }
        Action::WakeupEvent(_) => {
            while let Ok(msg) = store.service.actors().try_recv() {
                match msg {
                    ActorsMessageFrom::Shutdown => {
                        // TODO
                    }
                    ActorsMessageFrom::PeerStalled(_) => {
                        // TODO
                    }
                    ActorsMessageFrom::BlacklistPeer(_, _) => {
                        // TODO
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
