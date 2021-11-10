// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::{EnablingCondition, State};

/// Add incoming peer to peers.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersAddIncomingPeerAction {
    pub token: PeerToken,
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeersAddIncomingPeerAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
