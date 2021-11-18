// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::io_error_kind::IOErrorKind;
use crate::peer::PeerToken;
use crate::{EnablingCondition, State};

use super::PeerConnectionOutgoingStatePhase;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionOutgoingError {
    IO(IOErrorKind),
    Timeout(PeerConnectionOutgoingStatePhase),
}

impl From<std::io::Error> for PeerConnectionOutgoingError {
    fn from(error: std::io::Error) -> Self {
        Self::IO(error.kind().into())
    }
}

/// Initialize outgoing connection to a random potential peer.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingRandomInitAction {}

impl EnablingCondition<State> for PeerConnectionOutgoingRandomInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// Initialize outgoing connection to potential peer.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingInitAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionOutgoingInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingPendingAction {
    pub address: SocketAddr,
    pub token: PeerToken,
}

impl EnablingCondition<State> for PeerConnectionOutgoingPendingAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingErrorAction {
    pub address: SocketAddr,
    pub error: PeerConnectionOutgoingError,
}

impl EnablingCondition<State> for PeerConnectionOutgoingErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingSuccessAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionOutgoingSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
