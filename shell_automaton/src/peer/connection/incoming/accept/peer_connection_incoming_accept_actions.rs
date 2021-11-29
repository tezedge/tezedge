// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::peers::PeerBlacklistState;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::{EnablingCondition, State};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingRejectedReason {
    PeersConnectedMaxBoundReached,
    PeerBlacklisted(PeerBlacklistState),
}

impl EnablingCondition<State> for PeerConnectionIncomingRejectedReason {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// Accept incoming peer connection.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptAction {}

impl EnablingCondition<State> for PeerConnectionIncomingAcceptAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptErrorAction {
    pub error: PeerConnectionIncomingAcceptError,
}

impl EnablingCondition<State> for PeerConnectionIncomingAcceptErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingRejectedAction {
    pub token: PeerToken,
    pub address: SocketAddr,
    pub reason: PeerConnectionIncomingRejectedReason,
}

impl EnablingCondition<State> for PeerConnectionIncomingRejectedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptSuccessAction {
    pub token: PeerToken,
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionIncomingAcceptSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
