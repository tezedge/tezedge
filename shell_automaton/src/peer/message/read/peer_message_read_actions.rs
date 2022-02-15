// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::{EnablingCondition, State};

use super::PeerMessageReadError;

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadInitAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerMessageReadInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// PeerMessage has been read/received successfuly.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadErrorAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub error: PeerMessageReadError,
}

impl EnablingCondition<State> for PeerMessageReadErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// PeerMessage has been read/received successfuly.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadSuccessAction {
    #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
    pub address: SocketAddr,
    pub message: Arc<PeerMessageResponse>,
}

impl EnablingCondition<State> for PeerMessageReadSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
