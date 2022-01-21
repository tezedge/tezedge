// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::io_error_kind::IOErrorKind;
use crate::peer::PeerToken;
use crate::{EnablingCondition, State};

use super::PeerConnectionOutgoingStatePhase;

#[cfg(fuzzing)]
use crate::fuzzing::net::SocketAddrMutator;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
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
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingRandomInitAction {}

impl EnablingCondition<State> for PeerConnectionOutgoingRandomInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// Initialize outgoing connection to potential peer.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingInitAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionOutgoingInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingPendingAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub token: PeerToken,
}

impl EnablingCondition<State> for PeerConnectionOutgoingPendingAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingErrorAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub error: PeerConnectionOutgoingError,
}

impl EnablingCondition<State> for PeerConnectionOutgoingErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionOutgoingSuccessAction {
    #[cfg_attr(fuzzing, field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionOutgoingSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
