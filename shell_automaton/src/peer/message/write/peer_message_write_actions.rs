// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::{EnablingCondition, State};

use super::PeerMessageWriteError;

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteNextAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerMessageWriteNextAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteInitAction {
    pub address: SocketAddr,
    pub message: Arc<PeerMessageResponse>,
}

impl EnablingCondition<State> for PeerMessageWriteInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteErrorAction {
    pub address: SocketAddr,
    pub error: PeerMessageWriteError,
}

impl EnablingCondition<State> for PeerMessageWriteErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// PeerMessage has been read/received successfuly.
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteSuccessAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerMessageWriteSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
