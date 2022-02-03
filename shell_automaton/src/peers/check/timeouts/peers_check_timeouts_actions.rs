// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::PeersTimeouts;

//#[cfg(fuzzing)]
//use super::PeersTimeoutsMutator;
#[cfg(fuzzing)]
use crate::fuzzing::net::IpAddrMutator;
#[cfg(fuzzing)]
use fuzzcheck::mutators::vector::VecMutator;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsInitAction {}

impl EnablingCondition<State> for PeersCheckTimeoutsInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsSuccessAction {
    //#[cfg_attr(fuzzing, field_mutator(PeersTimeoutsMutator))]
    pub peer_timeouts: PeersTimeouts,
    #[cfg_attr(fuzzing, field_mutator(VecMutator<IpAddr, IpAddrMutator>))]
    pub graylist_timeouts: Vec<IpAddr>,
}

impl EnablingCondition<State> for PeersCheckTimeoutsSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsCleanupAction {}

impl EnablingCondition<State> for PeersCheckTimeoutsCleanupAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
