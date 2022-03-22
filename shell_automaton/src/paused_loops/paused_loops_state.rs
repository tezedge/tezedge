// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#[cfg(feature = "fuzzing")]
use crate::fuzzing::net::SocketAddrMutator;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::SocketAddr;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PausedLoop {
    PeerTryWrite {
        #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
        peer_address: SocketAddr,
    },
    PeerTryRead {
        #[cfg_attr(feature = "fuzzing", field_mutator(SocketAddrMutator))]
        peer_address: SocketAddr,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PausedLoopCurrent {
    None,
    Init(PausedLoop),
    Success,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PausedLoopsState {
    list: VecDeque<PausedLoop>,
    pub(super) current: PausedLoopCurrent,
}

impl PausedLoopsState {
    #[inline(always)]
    pub fn new() -> Self {
        Self {
            list: VecDeque::new(),
            current: PausedLoopCurrent::None,
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    #[inline(always)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    #[inline(always)]
    pub(super) fn add(&mut self, puased_loop: PausedLoop) {
        self.list.push_back(puased_loop)
    }

    #[inline(always)]
    pub(super) fn pop_front(&mut self) -> Option<PausedLoop> {
        self.list.pop_front()
    }
}

impl Default for PausedLoopsState {
    fn default() -> Self {
        Self::new()
    }
}
