// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::{EnablingCondition, State};

use super::PeerMessageReadError;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadInitAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerMessageReadInitAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// PeerMessage has been read/received successfuly.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadErrorAction {
    pub address: SocketAddr,
    pub error: PeerMessageReadError,
}

impl EnablingCondition<State> for PeerMessageReadErrorAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}

/// PeerMessage has been read/received successfuly.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageReadSuccessAction {
    pub address: SocketAddr,
    pub message: Arc<PeerMessageResponse>,
}

impl EnablingCondition<State> for PeerMessageReadSuccessAction {
    fn is_enabled(&self, _: &State) -> bool {
        true
    }
}
