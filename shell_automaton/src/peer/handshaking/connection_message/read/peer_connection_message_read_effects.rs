use crypto::crypto_box::{CryptoKey, PublicKey};
use redux_rs::{ActionWithId, Store};
use std::net::SocketAddr;
use tezos_messages::p2p::encoding::connection::ConnectionMessage;

use tezos_messages::p2p::binary_message::BinaryRead;

use crate::action::Action;
use crate::peer::handshaking::connection_message::read::PeerConnectionMessageReadSuccessAction;
use crate::peer::handshaking::{
    ConnectionMessageDataReceived, MessageReadState, PeerHandshakingStatus,
};
use crate::peer::{PeerStatus, PeerTryReadAction};
use crate::service::Service;
use crate::State;

pub fn peer_connection_message_read_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerConnectionMessageReadInit(action) => {
            store.dispatch(
                PeerTryReadAction {
                    address: action.address,
                }
                .into(),
            );
        }
        Action::PeerConnectionMessagePartRead(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(peer) => peer,
                None => return,
            };
            let msg_read_state = match &peer.status {
                PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                    PeerHandshakingStatus::ConnectionMessageRead { status, .. } => status,
                    _ => return,
                },
                _ => return,
            };

            match msg_read_state {
                MessageReadState::Pending { buffer } => {
                    if let Some(bytes) = buffer.peek_if_ready() {
                        let conn_msg = match ConnectionMessage::from_bytes(bytes.content()) {
                            Ok(v) => v,
                            Err(err) => todo!("handle error"),
                        };
                        let public_key = match PublicKey::from_bytes(conn_msg.public_key()) {
                            Ok(v) => v,
                            Err(err) => todo!("handle error"),
                        };
                        store.dispatch(
                            PeerConnectionMessageReadSuccessAction {
                                address: action.address,
                                port: conn_msg.port,
                                compatible_version: store
                                    .state
                                    .get()
                                    .config
                                    .shell_compatibility_version
                                    .choose_compatible_version(conn_msg.version()),
                                public_key,
                            }
                            .into(),
                        );
                    } else {
                        store.dispatch(
                            PeerTryReadAction {
                                address: action.address,
                            }
                            .into(),
                        );
                    }
                }
                _ => return,
            }
        }
        _ => {}
    }
}
