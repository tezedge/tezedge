// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

/// Do dns lookups to find and connect to initial peers.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersInitAction {}

impl EnablingCondition<State> for PeersInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state.peers.len() == 0
    }
}
