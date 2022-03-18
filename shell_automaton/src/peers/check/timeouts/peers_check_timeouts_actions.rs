// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::IpAddr;

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::{PeersCheckTimeoutsState, PeersTimeouts};

// TODO: add Default for BasicEnum in fuzzcheck-rs and uncomment this code
//#[cfg(feature = "fuzzing")]
//use super::PeersTimeoutsMutator;
#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::IpAddrMutator;
#[cfg(feature = "fuzzing")]
use fuzzcheck::mutators::vector::VecMutator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsInitAction {}

impl EnablingCondition<State> for PeersCheckTimeoutsInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        match &state.peers.check_timeouts {
            PeersCheckTimeoutsState::Idle { time } => {
                let check_timeouts_interval =
                    state.config.check_timeouts_interval.as_nanos() as u64;

                state.time_as_nanos() - time >= check_timeouts_interval
            }
            _ => false,
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsSuccessAction {
    // TODO: add Default for BasicEnum in fuzzcheck-rs and uncomment this code
    //#[cfg_attr(feature = "fuzzing", field_mutator(PeersTimeoutsMutator))]
    pub peer_timeouts: PeersTimeouts,
    #[cfg_attr(feature = "fuzzing", field_mutator(VecMutator<IpAddr, IpAddrMutator>))]
    pub graylist_timeouts: Vec<IpAddr>,
}

impl EnablingCondition<State> for PeersCheckTimeoutsSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersCheckTimeoutsCleanupAction {}

impl EnablingCondition<State> for PeersCheckTimeoutsCleanupAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
