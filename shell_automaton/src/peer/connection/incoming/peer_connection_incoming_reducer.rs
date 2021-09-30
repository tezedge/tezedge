use redux_rs::ActionWithId;

use crate::peer::connection::PeerConnectionState;
use crate::{action::Action, peer::PeerStatus, State};

use super::PeerConnectionIncomingState;

pub fn peer_connection_incoming_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerConnectionIncomingSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Connecting(PeerConnectionState::Incoming(
                    PeerConnectionIncomingState::Pending { token },
                )) = peer.status
                {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionIncomingState::Success { token }.into(),
                    );
                }
            }
        }
        _ => {}
    }
}
