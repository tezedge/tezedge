// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;

use crate::peer::connection::incoming::{
    PeerConnectionIncomingError, PeerConnectionIncomingErrorAction,
};
use crate::peer::connection::outgoing::{
    PeerConnectionOutgoingError, PeerConnectionOutgoingErrorAction,
};
use crate::peer::connection::PeerConnectionStatePhase;
use crate::peer::disconnection::PeerDisconnectAction;
use crate::peer::handshaking::{PeerHandshakingError, PeerHandshakingErrorAction};
use crate::peer::{Peer, PeerStatus};
use crate::peers::graylist::PeersGraylistIpRemoveAction;
use crate::{Action, ActionWithMeta, Service, Store};

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
        PeerStatus::Connecting(connecting) => {
            if current_time < connecting.time() + peer_connecting_timeout {
                return None;
            }
            PeerTimeout::Connecting(connecting.into())
        }
        PeerStatus::Handshaking(handshaking) => {
            if current_time < handshaking.since + peer_handshaking_timeout {
                return None;
            }
            PeerTimeout::Handshaking((&handshaking.status).into())
        }
        PeerStatus::Handshaked(peer) => {
            if let Some(current_head_last_update) = peer.current_head_last_update {
                if current_time - current_head_last_update
                    < Duration::from_secs(120).as_nanos() as u64
                {
                    return None;
                }
            } else {
                if current_time - peer.handshaked_since < Duration::from_secs(8).as_nanos() as u64 {
                    return None;
                }
            }
            PeerTimeout::CurrentHeadUpdate
        }
        PeerStatus::Disconnecting(_) => return None,
        PeerStatus::Disconnected => return None,
    })
}

pub fn peers_check_timeouts_effects<S>(store: &mut Store<S>, action: &ActionWithMeta)
where
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

            let peer_timeouts = state
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

            let graylist_timeouts = state
                .peers
                .blacklist_ip_iter()
                .filter_map(|(ip, blacklisted)| {
                    if blacklisted
                        .timeout(state.config.peers_graylist_timeout)
                        .filter(|timeout| current_time >= *timeout)
                        .is_some()
                    {
                        Some(*ip)
                    } else {
                        None
                    }
                })
                .collect();

            store.dispatch(PeersCheckTimeoutsSuccessAction {
                peer_timeouts,
                graylist_timeouts,
            });
        }
        Action::PeersCheckTimeoutsSuccess(_) => {
            match &state.peers.check_timeouts {
                PeersCheckTimeoutsState::Success {
                    peer_timeouts,
                    graylist_timeouts,
                    ..
                } => {
                    let graylist_timeouts = graylist_timeouts.clone();

                    for (address, timeout) in peer_timeouts.clone() {
                        match timeout {
                            PeerTimeout::Connecting(connecting) => match connecting {
                                PeerConnectionStatePhase::Incoming(incoming) => {
                                    store.dispatch(PeerConnectionIncomingErrorAction {
                                        address,
                                        error: PeerConnectionIncomingError::Timeout(incoming),
                                    });
                                }
                                PeerConnectionStatePhase::Outgoing(outgoing) => {
                                    store.dispatch(PeerConnectionOutgoingErrorAction {
                                        address,
                                        error: PeerConnectionOutgoingError::Timeout(outgoing),
                                    });
                                }
                            },
                            PeerTimeout::Handshaking(timeout) => {
                                store.dispatch(PeerHandshakingErrorAction {
                                    address,
                                    error: PeerHandshakingError::Timeout(timeout),
                                });
                            }
                            PeerTimeout::CurrentHeadUpdate => {
                                store.dispatch(PeerDisconnectAction { address });
                            }
                        }
                    }

                    for ip in graylist_timeouts {
                        store.dispatch(PeersGraylistIpRemoveAction { ip: ip.clone() });
                    }
                }
                _ => return,
            }
            store.dispatch(PeersCheckTimeoutsCleanupAction {});
        }
        _ => match &state.peers.check_timeouts {
            PeersCheckTimeoutsState::Idle { time } => {
                let check_timeouts_interval =
                    state.config.check_timeouts_interval.as_nanos() as u64;

                if current_time - time >= check_timeouts_interval {
                    store.dispatch(PeersCheckTimeoutsInitAction {});
                }
            }
            _ => {}
        },
    }
}
