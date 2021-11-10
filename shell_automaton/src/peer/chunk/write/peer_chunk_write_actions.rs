// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use tezos_messages::p2p::binary_message::BinaryChunk;

use crate::{EnablingCondition, State};

use super::PeerChunkWriteError;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteSetContentAction {
    pub address: SocketAddr,
    pub content: Vec<u8>,
}

impl EnablingCondition<State> for PeerChunkWriteSetContentAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteEncryptContentAction {
    pub address: SocketAddr,
    pub encrypted_content: Vec<u8>,
}

impl EnablingCondition<State> for PeerChunkWriteEncryptContentAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteCreateChunkAction {
    pub address: SocketAddr,
    pub chunk: BinaryChunk,
}

impl EnablingCondition<State> for PeerChunkWriteCreateChunkAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWritePartAction {
    pub address: SocketAddr,
    pub written: usize,
}

impl EnablingCondition<State> for PeerChunkWritePartAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteErrorAction {
    pub address: SocketAddr,
    pub error: PeerChunkWriteError,
}

impl EnablingCondition<State> for PeerChunkWriteErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWriteReadyAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerChunkWriteReadyAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
