// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};

use crate::peer::connection::incoming::{
    PeerConnectionIncomingState, PeerConnectionIncomingStatePhase,
};
use crate::peer::connection::outgoing::{
    PeerConnectionOutgoingState, PeerConnectionOutgoingStatePhase,
};
use crate::peer::PeerToken;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionState {
    Outgoing(PeerConnectionOutgoingState),
    Incoming(PeerConnectionIncomingState),
}

impl PeerConnectionState {
    pub fn token(&self) -> Option<PeerToken> {
        match self {
            Self::Outgoing(s) => s.token(),
            Self::Incoming(s) => Some(s.token()),
        }
    }

    pub fn time(&self) -> u64 {
        match self {
            Self::Outgoing(s) => s.time(),
            Self::Incoming(s) => s.time(),
        }
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(From, Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionStatePhase {
    Outgoing(PeerConnectionOutgoingStatePhase),
    Incoming(PeerConnectionIncomingStatePhase),
}

impl<'a> From<&'a PeerConnectionState> for PeerConnectionStatePhase {
    fn from(state: &'a PeerConnectionState) -> Self {
        match state {
            PeerConnectionState::Outgoing(v) => Self::Outgoing(v.into()),
            PeerConnectionState::Incoming(v) => Self::Incoming(v.into()),
        }
    }
}
