// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{EnablingCondition, State};

use super::PeerIOLoopResult;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryWriteLoopStartAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerTryWriteLoopStartAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryWriteLoopFinishAction {
    pub address: SocketAddr,
    pub result: PeerIOLoopResult,
}

impl EnablingCondition<State> for PeerTryWriteLoopFinishAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryReadLoopStartAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerTryReadLoopStartAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryReadLoopFinishAction {
    pub address: SocketAddr,
    pub result: PeerIOLoopResult,
}

impl EnablingCondition<State> for PeerTryReadLoopFinishAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
