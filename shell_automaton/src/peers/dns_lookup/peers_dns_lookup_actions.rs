// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{EnablingCondition, Port, State};

use super::DnsLookupError;

#[cfg(fuzzing)]
use crate::fuzzing::net::SocketAddrMutator;
#[cfg(fuzzing)]
use fuzzcheck::mutators::vector::VecMutator;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersDnsLookupInitAction {
    pub address: String,
    pub port: Port,
}

impl EnablingCondition<State> for PeersDnsLookupInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersDnsLookupErrorAction {
    pub error: DnsLookupError,
}

impl EnablingCondition<State> for PeersDnsLookupErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersDnsLookupSuccessAction {
    #[cfg_attr(fuzzing, field_mutator(VecMutator<SocketAddr, SocketAddrMutator>))]
    pub addresses: Vec<SocketAddr>,
}

impl EnablingCondition<State> for PeersDnsLookupSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// Cleanup dns lookup state.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersDnsLookupCleanupAction;

impl EnablingCondition<State> for PeersDnsLookupCleanupAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
