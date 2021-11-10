// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crate::{EnablingCondition, State};

use super::PeerBinaryMessageReadError;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadInitAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerBinaryMessageReadInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadChunkReadyAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerBinaryMessageReadChunkReadyAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadSizeReadyAction {
    pub address: SocketAddr,
    pub size: usize,
}

impl EnablingCondition<State> for PeerBinaryMessageReadSizeReadyAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadReadyAction {
    pub address: SocketAddr,
    pub message: Vec<u8>,
}

impl EnablingCondition<State> for PeerBinaryMessageReadReadyAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerBinaryMessageReadErrorAction {
    pub address: SocketAddr,
    pub error: PeerBinaryMessageReadError,
}

impl EnablingCondition<State> for PeerBinaryMessageReadErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
