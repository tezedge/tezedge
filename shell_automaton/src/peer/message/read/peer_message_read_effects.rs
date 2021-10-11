use networking::network_channel::PeerMessageReceived;
use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::binary_message::BinaryRead;
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};

use crate::peer::binary_message::read::PeerBinaryMessageReadInitAction;
use crate::peer::PeerStatus;
use crate::peers::add::multi::PeersAddMultiAction;
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
                Ok(mut message) => {
                    // Set size hint to unencrypted encoded message size.
                    // Maybe we should set encrypted size instead? Since
                    // that's the actual size of data transmitted.
                    message.set_size_hint(action.message.len());

                    store.dispatch(
                        PeerMessageReadSuccessAction {
                            address: action.address,
                            message: message.into(),
                        }
                        .into(),
                    );
                }
                Err(err) => {
                    eprintln!("TODO: encountered PeerMessageResponse decode error handling of which not implemented!");
                }
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

            match &action.message.message() {
                PeerMessage::Advertise(msg) => {
                    store.dispatch(
                        PeersAddMultiAction {
                            addresses: msg.id().iter().filter_map(|x| x.parse().ok()).collect(),
                        }
                        .into(),
                    );
                }
                _ => {}
            }

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
