use networking::network_channel::PeerMessageReceived;
use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::binary_message::BinaryRead;
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::peer::binary_message::read::PeerBinaryMessageReadInitAction;
use crate::peer::PeerStatus;
use crate::service::actors_service::{ActorsMessageTo, ActorsService};
use crate::service::Service;
use crate::{Action, State};

use super::{PeerMessageReadInitAction, PeerMessageReadSuccessAction};

pub fn peer_message_read_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerMessageReadInit(action) => {
            store.dispatch(
                PeerBinaryMessageReadInitAction {
                    address: action.address,
                }
                .into(),
            );
        }
        Action::PeerBinaryMessageReadReady(action) => {
            let peer = match store.state().peers.get(&action.address) {
                Some(peer) => match peer.status.as_handshaked() {
                    Some(handshaked) => handshaked,
                    None => return,
                },
                None => return,
            };

            match PeerMessageResponse::from_bytes(&action.message) {
                Ok(message) => store.dispatch(
                    PeerMessageReadSuccessAction {
                        address: action.address,
                        message: message.into(),
                    }
                    .into(),
                ),
                Err(err) => todo!("handle PeerMessageResponse decode error"),
            }
        }
        Action::PeerMessageReadSuccess(action) => {
            store
                .service()
                .actors()
                .send(ActorsMessageTo::PeerMessageReceived(PeerMessageReceived {
                    peer_address: action.address,
                    message: action.message.clone(),
                }));

            // try to read next message.
            store.dispatch(
                PeerMessageReadInitAction {
                    address: action.address,
                }
                .into(),
            );
        }
        _ => {}
    }
}
