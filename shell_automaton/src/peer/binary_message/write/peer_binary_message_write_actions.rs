// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::PeerBinaryMessageWriteError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteSetContentAction {
    pub address: SocketAddr,
    pub message: Vec<u8>,
}

impl EnablingCondition<State> for PeerBinaryMessageWriteSetContentAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteNextChunkAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerBinaryMessageWriteNextChunkAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteReadyAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerBinaryMessageWriteReadyAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageWriteErrorAction {
    pub address: SocketAddr,
    pub error: PeerBinaryMessageWriteError,
}

impl EnablingCondition<State> for PeerBinaryMessageWriteErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
