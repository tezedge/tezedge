// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use super::PeerIOLoopResult;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryWriteLoopStartAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryWriteLoopFinishAction {
    pub address: SocketAddr,
    pub result: PeerIOLoopResult,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryReadLoopStartAction {
    pub address: SocketAddr,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerTryReadLoopFinishAction {
    pub address: SocketAddr,
    pub result: PeerIOLoopResult,
}
