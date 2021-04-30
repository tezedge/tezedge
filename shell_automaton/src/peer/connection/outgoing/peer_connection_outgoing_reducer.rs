use redux_rs::ActionWithId;

use crate::peer::connection::PeerConnectionState;
use crate::{
    action::Action,
    peer::{Peer, PeerStatus},
    State,
};

use super::PeerConnectionOutgoingState;

pub fn peer_connection_outgoing_reducer(state: &mut State, action: &ActionWithId<Action>) {
    match &action.action {
        Action::PeerConnectionOutgoingInit(action) => {
            let peer = state.peers.entry(action.address).or_insert_with(|| Peer {
                status: PeerStatus::Potential,
            });
            if matches!(peer.status, PeerStatus::Potential) {
                peer.status = PeerStatus::Connecting(PeerConnectionOutgoingState::Idle.into());
            }
        }
        Action::PeerConnectionOutgoingPending(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if matches!(
                    peer.status,
                    PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Idle
                    ))
                ) {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionOutgoingState::Pending {
                            token: action.token,
                        }
                        .into(),
                    );
                }
            }
        }
        Action::PeerConnectionOutgoingError(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if matches!(
                    peer.status,
                    PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Idle
                    )) | PeerStatus::Connecting(PeerConnectionState::Outgoing(
                        PeerConnectionOutgoingState::Pending { .. }
                    ))
                ) {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionOutgoingState::Error {
                            error: action.error,
                        }
                        .into(),
                    );
                }
            }
        }
        Action::PeerConnectionOutgoingSuccess(action) => {
            if let Some(peer) = state.peers.get_mut(&action.address) {
                if let PeerStatus::Connecting(PeerConnectionState::Outgoing(
                    PeerConnectionOutgoingState::Pending { token },
                )) = peer.status
                {
                    peer.status = PeerStatus::Connecting(
                        PeerConnectionOutgoingState::Success { token }.into(),
                    );
                }
            }
        }
        _ => {}
    }
}
