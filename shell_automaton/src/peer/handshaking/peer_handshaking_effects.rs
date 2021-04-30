use redux_rs::{ActionWithId, Store};
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryWrite};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;

use crate::action::Action;
use crate::peer::handshaking::connection_message::write::PeerConnectionMessageWriteInitAction;
use crate::peer::PeerStatus;
use crate::service::{RandomnessService, Service};
use crate::State;

use super::connection_message::read::PeerConnectionMessageReadInitAction;
use super::PeerHandshakingStatus;

pub fn peer_handshaking_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerHandshakingInit(action) => {
            let nonce = store.service().randomness().get_nonce(action.address);
            let config = &store.state().config;
            let conn_msg = ConnectionMessage::try_new(
                config.port,
                &config.identity.public_key,
                &config.identity.proof_of_work_stamp,
                nonce,
                config.shell_compatibility_version.to_network_version(),
            );

            let conn_msg = match conn_msg {
                Ok(msg) => msg,
                Err(err) => todo!("handle error"),
            };

            let encoded = match conn_msg.as_bytes() {
                Ok(encoded) => encoded,
                Err(err) => todo!("handle error"),
            };

            let binary_chunk = match BinaryChunk::from_content(&encoded) {
                Ok(chunk) => chunk,
                Err(err) => todo!("handle error"),
            };

            store.dispatch(
                PeerConnectionMessageWriteInitAction {
                    address: action.address,
                    conn_msg: binary_chunk,
                }
                .into(),
            );
        }
        Action::PeerConnectionMessageWriteSuccess(action) => {
            // check if sending ConnectionMessage was successful.
            let conn_msg_successfully_sent = store
                .state
                .get()
                .peers
                .get(&action.address)
                .and_then(|peer| match &peer.status {
                    PeerStatus::Handshaking(x) => Some(x),
                    _ => None,
                })
                .and_then(|handshaking| match &handshaking.status {
                    PeerHandshakingStatus::ConnectionMessageWrite { status, .. } => Some(status),
                    _ => None,
                })
                .map(|status| status.is_success());

            if conn_msg_successfully_sent.unwrap_or(false) {
                store.dispatch(
                    PeerConnectionMessageReadInitAction {
                        address: action.address,
                    }
                    .into(),
                );
            }
        }
        _ => {}
    }
}
