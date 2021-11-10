// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersRemoveAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeersRemoveAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
