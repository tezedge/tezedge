// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{EnablingCondition, State};

#[cfg(fuzzing)]
use crate::fuzzing::net::SocketAddrMutator;
#[cfg(fuzzing)]
use fuzzcheck::mutators::vector::VecMutator;

/// Add multiple peers as potential peers.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersAddMultiAction {
    #[cfg_attr(fuzzing, field_mutator(VecMutator<SocketAddr, SocketAddrMutator>))]
    pub addresses: Vec<SocketAddr>,
}

impl EnablingCondition<State> for PeersAddMultiAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
