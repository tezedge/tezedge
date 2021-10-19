use redux_rs::Store;

use crate::peer::connection::incoming::{
    PeerConnectionIncomingError, PeerConnectionIncomingErrorAction,
};
use crate::peer::connection::outgoing::{
    PeerConnectionOutgoingError, PeerConnectionOutgoingErrorAction,
};
use crate::peer::connection::PeerConnectionStatePhase;
use crate::peer::handshaking::{PeerHandshakingError, PeerHandshakingErrorAction};
use crate::peer::{Peer, PeerStatus};
use crate::{Action, ActionWithId, Service, State};

use super::{
    PeerTimeout, PeersCheckTimeoutsCleanupAction, PeersCheckTimeoutsInitAction,
    PeersCheckTimeoutsState, PeersCheckTimeoutsSuccessAction,
};

fn check_timeout(
    peer: &Peer,
    current_time: u64,
    peer_connecting_timeout: u64,
    peer_handshaking_timeout: u64,
) -> Option<PeerTimeout> {
    Some(match &peer.status {
        PeerStatus::Potential => return None,
        // TODO: detect connection timeouts as well.
        PeerStatus::Connecting(connecting) => {
            if current_time - connecting.time() < peer_connecting_timeout {
                return None;
            }
            PeerTimeout::Connecting(connecting.into())
        }
        PeerStatus::Handshaking(handshaking) => {
            if current_time - handshaking.since < peer_handshaking_timeout {
                return None;
            }
            PeerTimeout::Handshaking((&handshaking.status).into())
        }
        PeerStatus::Handshaked(_) => return None,
        PeerStatus::Disconnecting(_) => return None,
        PeerStatus::Disconnected => return None,
    })
}

pub fn peers_check_timeouts_effects<S>(
    store: &mut Store<State, S, Action>,
    action: &ActionWithId<Action>,
) where
    S: Service,
{
    let state = store.state.get();
    let current_time = state.time_as_nanos();

    match &action.action {
        Action::PeersCheckTimeoutsInit(_) => {
            if !matches!(
                &state.peers.check_timeouts,
                PeersCheckTimeoutsState::Init { .. }
            ) {
                return;
            }

            let peer_connecting_timeout = state.config.peer_connecting_timeout.as_nanos() as u64;
            let peer_handshaking_timeout = state.config.peer_handshaking_timeout.as_nanos() as u64;

            let timeouts = store
                .state
                .get()
                .peers
                .iter()
                .filter_map(|(address, peer)| {
                    let timeout = check_timeout(
                        peer,
                        current_time,
                        peer_connecting_timeout,
                        peer_handshaking_timeout,
                    )?;

                    Some((*address, timeout))
                })
                .collect();

            store.dispatch(PeersCheckTimeoutsSuccessAction { timeouts }.into());
        }
        Action::PeersCheckTimeoutsSuccess(_) => {
            match &state.peers.check_timeouts {
                PeersCheckTimeoutsState::Success { timeouts, .. } => {
                    for (address, timeout) in timeouts.clone() {
                        match timeout {
                            PeerTimeout::Connecting(connecting) => match connecting {
                                PeerConnectionStatePhase::Incoming(incoming) => {
                                    store.dispatch(
                                        PeerConnectionIncomingErrorAction {
                                            address,
                                            error: PeerConnectionIncomingError::Timeout(incoming),
                                        }
                                        .into(),
                                    );
                                }
                                PeerConnectionStatePhase::Outgoing(outgoing) => {
                                    store.dispatch(
                                        PeerConnectionOutgoingErrorAction {
                                            address,
                                            error: PeerConnectionOutgoingError::Timeout(outgoing),
                                        }
                                        .into(),
                                    );
                                }
                            },
                            PeerTimeout::Handshaking(timeout) => {
                                store.dispatch(
                                    PeerHandshakingErrorAction {
                                        address,
                                        error: PeerHandshakingError::Timeout(timeout),
                                    }
                                    .into(),
                                );
                            }
                        }
                    }
                }
                _ => return,
            }
            store.dispatch(PeersCheckTimeoutsCleanupAction {}.into());
        }
        _ => match &state.peers.check_timeouts {
            PeersCheckTimeoutsState::Idle { time } => {
                let check_timeouts_interval =
                    state.config.check_timeouts_interval.as_nanos() as u64;

                if current_time - time >= check_timeouts_interval {
                    store.dispatch(PeersCheckTimeoutsInitAction {}.into())
                }
            }
            _ => {}
        },
    }
}
