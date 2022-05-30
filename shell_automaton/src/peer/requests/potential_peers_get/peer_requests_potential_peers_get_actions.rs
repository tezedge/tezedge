// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::{PeerRequestsPotentialPeersGetError, MIN_PEER_POTENTIAL_PEERS_GET_DELAY};

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;
#[cfg(feature = "fuzzing")]
use fuzzcheck::mutators::vector::VecMutator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRequestsPotentialPeersGetInitAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl PeerRequestsPotentialPeersGetInitAction {
    pub fn should_request(state: &State) -> bool {
        if state.config.private_node {
            return false;
        }

        // only request from 1 peer at a time.
        if state
            .peers
            .handshaked_iter()
            .any(|(_, p)| p.requests.potential_peers_get.is_pending())
        {
            return false;
        }

        let potential_len = state.peers.potential_len();

        if potential_len >= state.config.peers_potential_max.max(1) {
            return false;
        }

        potential_len <= state.config.peers_connected_min.max(2)
    }

    pub fn can_request_from_peer(state: &State, peer: SocketAddr) -> bool {
        state
            .peers
            .get_handshaked(&peer)
            .map(|p| &p.requests.potential_peers_get)
            .filter(|req| !req.is_pending())
            .map_or(false, |req| {
                state.time_as_nanos() >= req.time() + MIN_PEER_POTENTIAL_PEERS_GET_DELAY
            })
    }
}

impl EnablingCondition<State> for PeerRequestsPotentialPeersGetInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        Self::should_request(state) && Self::can_request_from_peer(state, self.address)
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRequestsPotentialPeersGetPendingAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRequestsPotentialPeersGetPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map_or(false, |p| p.requests.potential_peers_get.is_init())
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRequestsPotentialPeersGetErrorAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub error: PeerRequestsPotentialPeersGetError,
}

impl EnablingCondition<State> for PeerRequestsPotentialPeersGetErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map_or(false, |p| p.requests.potential_peers_get.is_pending())
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRequestsPotentialPeersGetSuccessAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    #[cfg_attr(feature = "fuzzing", field_mutator(VecMutator<SocketAddr, SocketAddrMutator>))]
    pub result: Vec<SocketAddr>,
}

impl EnablingCondition<State> for PeerRequestsPotentialPeersGetSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map_or(false, |p| p.requests.potential_peers_get.is_pending())
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRequestsPotentialPeersGetFinishAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRequestsPotentialPeersGetFinishAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map_or(false, |p| p.requests.potential_peers_get.is_success())
    }
}
