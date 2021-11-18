// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::PeersTimeouts;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsInitAction {}

impl EnablingCondition<State> for PeersCheckTimeoutsInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsSuccessAction {
    pub peer_timeouts: PeersTimeouts,
    pub graylist_timeouts: Vec<IpAddr>,
}

impl EnablingCondition<State> for PeersCheckTimeoutsSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsCleanupAction {}

impl EnablingCondition<State> for PeersCheckTimeoutsCleanupAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
