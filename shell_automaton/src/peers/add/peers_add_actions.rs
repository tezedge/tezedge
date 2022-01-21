// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::{EnablingCondition, State};

#[cfg(fuzzing)]
use crate::fuzzing::net::SocketAddrMutator;

/// Add incoming peer to peers.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersAddIncomingPeerAction {
    pub token: PeerToken,
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeersAddIncomingPeerAction {
    fn is_enabled(&self, state: &State) -> bool {
        if state.peers.connected_len() >= state.config.peers_connected_max {
            return false;
        }

        if state.peers.is_blacklisted(&self.address.ip()) {
            return false;
        }

        true
    }
}
