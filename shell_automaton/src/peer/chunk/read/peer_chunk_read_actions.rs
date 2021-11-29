// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{EnablingCondition, State};

use super::PeerChunkReadError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadInitAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerChunkReadInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadPartAction {
    pub address: SocketAddr,
    pub bytes: Vec<u8>,
}

impl EnablingCondition<State> for PeerChunkReadPartAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadDecryptAction {
    pub address: SocketAddr,
    pub decrypted_bytes: Vec<u8>,
}

impl EnablingCondition<State> for PeerChunkReadDecryptAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadReadyAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerChunkReadReadyAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkReadErrorAction {
    pub address: SocketAddr,
    pub error: PeerChunkReadError,
}

impl EnablingCondition<State> for PeerChunkReadErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
