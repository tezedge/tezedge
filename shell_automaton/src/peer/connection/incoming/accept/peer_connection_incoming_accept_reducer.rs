use redux_rs::ActionWithId;

use crate::action::Action;
use crate::peer::{Peer, PeerStatus};
use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::State;

use super::PeerConnectionIncomingAcceptState;

pub fn peer_connection_incoming_accept_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerConnectionIncomingAcceptSuccess(action) => {
            match &state.peer_connection_incoming_accept {
                PeerConnectionIncomingAcceptState::Idle => {}
                _ => return,
            }
            state.peer_connection_incoming_accept = PeerConnectionIncomingAcceptState::Success {
                token: action.token,
                address: action.address,
            };
        }
        Action::PeerConnectionIncomingAcceptError(action) => {
            if matches!(&action.error, PeerConnectionIncomingAcceptError::WouldBlock) {
                return;
            }
            state.peer_connection_incoming_accept = PeerConnectionIncomingAcceptState::Error {
                error: action.error.clone(),
            };
        }
        _ => {}
    }
}
