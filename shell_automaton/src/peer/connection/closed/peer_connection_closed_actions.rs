// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{EnablingCondition, State};

#[cfg(fuzzing)]
use crate::fuzzing::net::SocketAddrMutator;

/// Peer closed connection with us.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionClosedAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionClosedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
