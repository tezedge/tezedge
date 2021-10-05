use redux_rs::{ActionWithId, Store};
use std::sync::Arc;

use tezos_messages::p2p::encoding::metadata::MetadataMessage;

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
            let peer = match store.state.get().peers.get(&action.address) {
                Some(v) => v,
                None => return,
            };
            let peer = match &peer.status {
                PeerStatus::Handshaked(v) => v,
                _ => return,
            };

            let peer_id = Arc::new(PeerId {
                address: action.address,
                public_key_hash: peer.public_key_hash.clone(),
            });

            let meta_msg = MetadataMessage::new(peer.disable_mempool, peer.private_node);
            let version = Arc::new(peer.version.clone());

            store
                .service
                .actors()
                .send(ActorsMessageTo::PeerHandshaked(peer_id, meta_msg, version));
        }
        _ => {}
    }
}
