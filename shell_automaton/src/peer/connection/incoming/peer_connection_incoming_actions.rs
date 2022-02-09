// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::{EnablingCondition, State};

use super::PeerConnectionIncomingStatePhase;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionIncomingError {
    Timeout(PeerConnectionIncomingStatePhase),
}

impl EnablingCondition<State> for PeerConnectionIncomingError {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingErrorAction {
    pub address: SocketAddr,
    pub error: PeerConnectionIncomingError,
}

impl EnablingCondition<State> for PeerConnectionIncomingErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionIncomingSuccessAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerConnectionIncomingSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
