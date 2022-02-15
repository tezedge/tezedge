// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;
#[cfg(feature = "fuzzing")]
use fuzzcheck::{
    mutators::{
        tuples::{Tuple2, Tuple2Mutator, TupleMutatorWrapper},
        vector::VecMutator,
    },
    DefaultMutator,
};

use crate::peer::connection::PeerConnectionStatePhase;
use crate::peer::handshaking::PeerHandshakingPhase;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, SocketAddr};

pub type PeersTimeouts = Vec<(SocketAddr, PeerTimeout)>;

pub type GraylistTimeouts = Vec<IpAddr>;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerTimeout {
    Connecting(PeerConnectionStatePhase),
    Handshaking(PeerHandshakingPhase),
}

#[cfg(feature = "fuzzing")]
pub type PeersTimeoutsMutator = VecMutator<
    (SocketAddr, PeerTimeout),
    TupleMutatorWrapper<
        Tuple2Mutator<
            SocketAddrMutator,
            PeerTimeoutMutator<
                <PeerConnectionStatePhase as DefaultMutator>::Mutator,
                <PeerHandshakingPhase as DefaultMutator>::Mutator,
            >,
        >,
        Tuple2<SocketAddr, PeerTimeout>,
    >,
>;

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
