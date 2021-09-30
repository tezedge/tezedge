use redux_rs::{ActionWithId, Store};

use crate::action::Action;
use crate::peer::handshaking::{MessageWriteState, PeerHandshakingStatus};
use crate::peer::{PeerStatus, PeerTryWriteAction};
use crate::service::Service;
use crate::State;

use super::PeerConnectionMessageWriteSuccessAction;

pub fn peer_connection_message_write_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    match &action.action {
        Action::PeerConnectionMessageWriteInit(action) => {
            store.dispatch(
                PeerTryWriteAction {
                    address: action.address,
                }
                .into(),
            );
        }
        Action::PeerConnectionMessagePartWritten(action) => {
            let peer = match store.state.get().peers.get(&action.address) {
                Some(peer) => peer,
                None => return,
            };
            let (conn_msg, msg_write_state) = match &peer.status {
                PeerStatus::Handshaking(handshaking) => match &handshaking.status {
                    PeerHandshakingStatus::ConnectionMessageWrite { conn_msg, status } => {
                        (conn_msg, status)
                    }
                    _ => return,
                },
                _ => return,
            };

            match msg_write_state {
                MessageWriteState::Pending { written } => {
                    if *written == conn_msg.raw().len() {
                        store.dispatch(
                            PeerConnectionMessageWriteSuccessAction {
                                address: action.address,
                            }
                            .into(),
                        );
                    } else {
                        // Message is not yet fully written, so try to write rest of it.
                        store.dispatch(
                            PeerTryWriteAction {
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
