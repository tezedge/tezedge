// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;
use crate::peers::PeerBlacklistState;
use crate::service::mio_service::PeerConnectionIncomingAcceptError;
use crate::{EnablingCondition, State};

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::{PeerTokenMutator, SocketAddrMutator};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
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
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptAction {}

impl EnablingCondition<State> for PeerConnectionIncomingAcceptAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptErrorAction {
    pub error: PeerConnectionIncomingAcceptError,
}

impl EnablingCondition<State> for PeerConnectionIncomingAcceptErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingRejectedAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
    pub token: PeerToken,
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub reason: PeerConnectionIncomingRejectedReason,
}

impl EnablingCondition<State> for PeerConnectionIncomingRejectedAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingAcceptSuccessAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(PeerTokenMutator))]
    pub token: PeerToken,
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionIncomingAcceptSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
