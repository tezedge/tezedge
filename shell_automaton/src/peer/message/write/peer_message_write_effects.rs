use networking::network_channel::PeerMessageReceived;
use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::binary_message::{BinaryRead, BinaryWrite};
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::peer::binary_message::write::{
    PeerBinaryMessageWriteSetContentAction, PeerBinaryMessageWriteState,
};
use crate::peer::message::write::PeerMessageWriteSuccessAction;
use crate::peer::PeerStatus;
use crate::service::actors_service::{ActorsMessageTo, ActorsService};
use crate::service::Service;
use crate::{Action, State};

use super::{PeerMessageWriteInitAction, PeerMessageWriteNextAction};

pub fn peer_message_write_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerMessageWriteNext(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(v) => v,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Init { .. } = &peer.message_write.current {
                if let Some(front_msg) = peer.message_write.queue.front() {
                    store.dispatch(
                        PeerMessageWriteInitAction {
                            address: action.address,
                            message: front_msg.clone(),
                        }
                        .into(),
                    );
                }
            }
        }
        Action::PeerMessageWriteInit(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(v) => v,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Init { .. } = &peer.message_write.current {
                match action.message.as_bytes() {
                    Ok(bytes) => store.dispatch(
                        PeerBinaryMessageWriteSetContentAction {
                            address: action.address,
                            message: bytes,
                        }
                        .into(),
                    ),
                    Err(err) => {
                        eprintln!("TODO: encountered PeerMessageResponse encode error handling of which not implemented!");
                    }
                }
            }
        }
        Action::PeerBinaryMessageWriteReady(action) => {
            let peer = match store.state().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(handshaked) => handshaked,
                    None => return,
                },
                None => return,
            };

            if let PeerBinaryMessageWriteState::Ready { .. } = &peer.message_write.current {
                store.dispatch(
                    PeerMessageWriteSuccessAction {
                        address: action.address,
                    }
                    .into(),
                );
            }
        }
        Action::PeerMessageWriteSuccess(action) => {
            store.dispatch(
                PeerMessageWriteNextAction {
                    address: action.address,
                }
                .into(),
            );
        }
        _ => {}
    }
}
