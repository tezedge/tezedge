use redux_rs::Store;

use crate::peer::handshaking::{PeerHandshakingError, PeerHandshakingErrorAction};
use crate::peer::PeerStatus;
use crate::{Action, ActionWithId, Service, State};

use super::{
    PeerTimeout, PeersCheckTimeoutsCleanupAction, PeersCheckTimeoutsInitAction,
    PeersCheckTimeoutsState, PeersCheckTimeoutsSuccessAction,
};

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

            let handshaking_timeout = state.config.peer_handshaking_timeout.as_nanos() as u64;

            let timeouts = store
                .state
                .get()
                .peers
                .iter()
                .filter_map(|(address, peer)| {
                    let timeout = match &peer.status {
                        PeerStatus::Potential => return None,
                        // TODO: detect connection timeouts as well.
                        PeerStatus::Connecting(_) => return None,
                        PeerStatus::Handshaking(handshaking) => {
                            if current_time - handshaking.since < handshaking_timeout {
                                return None;
                            }
                            PeerTimeout::Handshaking((&handshaking.status).into())
                        }
                        PeerStatus::Handshaked(_) => return None,
                        PeerStatus::Disconnecting(_) => return None,
                        PeerStatus::Disconnected => return None,
                    };

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
