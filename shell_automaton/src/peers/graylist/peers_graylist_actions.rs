// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

use crate::{EnablingCondition, State};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistAddressAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeersGraylistAddressAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpAddAction {
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpAddAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpAddedAction {
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpAddedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpRemoveAction {
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpRemoveAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersGraylistIpRemovedAction {
    pub ip: IpAddr,
}

impl EnablingCondition<State> for PeersGraylistIpRemovedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
