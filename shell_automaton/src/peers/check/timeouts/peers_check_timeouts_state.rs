// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};

use crate::peer::connection::PeerConnectionStatePhase;
use crate::peer::handshaking::PeerHandshakingPhase;

pub type PeersTimeouts = BTreeMap<SocketAddr, PeerTimeout>;
pub type GraylistTimeouts = Vec<IpAddr>;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerTimeout {
    Connecting(PeerConnectionStatePhase),
    Handshaking(PeerHandshakingPhase),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeersCheckTimeoutsState {
    Idle {
        time: u64,
    },
    Init {
        time: u64,
    },
    Success {
        time: u64,
        peer_timeouts: PeersTimeouts,
        graylist_timeouts: GraylistTimeouts,
    },
}

impl PeersCheckTimeoutsState {
    pub fn new() -> Self {
        Self::Idle { time: 0 }
    }
}
