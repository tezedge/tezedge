// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

use crate::{EnablingCondition, State};

#[cfg(fuzzing)]
use crate::fuzzing::net::{IpAddrMutator, SocketAddrMutator};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistAddressAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeersGraylistAddressAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpAddAction {
    #[cfg_attr(fuzzing, field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpAddAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpAddedAction {
    #[cfg_attr(fuzzing, field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpAddedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpRemoveAction {
    #[cfg_attr(fuzzing, field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpRemoveAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpRemovedAction {
    #[cfg_attr(fuzzing, field_mutator(IpAddrMutator))]
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpRemovedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
