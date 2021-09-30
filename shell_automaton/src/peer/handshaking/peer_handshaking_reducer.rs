use redux_rs::ActionWithId;

use crate::{
    action::Action,
    peer::{
        connection::{
            incoming::PeerConnectionIncomingState, outgoing::PeerConnectionOutgoingState,
            PeerConnectionState,
        },
        Peer, PeerStatus,
    },
    State,
};

use super::{PeerHandshaking, PeerHandshakingStatus};

pub fn peer_handshaking_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerHandshakingInit(action) => {
            let peer = match state.peers.get_mut(&action.address) {
                Some(v) => v,
                None => return,
            };

            let conn_state = match &peer.status {
                PeerStatus::Connecting(v) => v,
                _ => return,
            };

            peer.status = match conn_state {
                PeerConnectionState::Incoming(PeerConnectionIncomingState::Success { token }) => {
                    PeerStatus::Handshaking(PeerHandshaking {
                        token: *token,
                        incoming: true,
                        status: PeerHandshakingStatus::Init,
                    })
                }
                PeerConnectionState::Outgoing(PeerConnectionOutgoingState::Success { token }) => {
                    PeerStatus::Handshaking(PeerHandshaking {
                        token: *token,
                        incoming: false,
                        status: PeerHandshakingStatus::Init,
                    })
                }
                _ => return,
            };
        }
        _ => {}
    }
}
