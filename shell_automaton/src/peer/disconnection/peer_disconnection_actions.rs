// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{EnablingCondition, State};

/// Disconnect connected peer.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerDisconnectAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerDisconnectAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// Connected peer disconnected.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerDisconnectedAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerDisconnectedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
