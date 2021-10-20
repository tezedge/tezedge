// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! This crate is backport of handles low level p2p communication.

use std::sync::Arc;

pub(crate) use peer::*;
use shell::ShellCompatibilityVersion;
pub(crate) use stream::*;
use tezos_identity::Identity;

mod peer;
mod stream;

/// Local peer info
pub struct LocalPeerInfo {
    /// port where remote node can establish new connection
    listener_port: u16,
    /// Our node identity
    identity: Arc<Identity>,
    /// version of shell/network protocol which we are compatible with
    version: Arc<ShellCompatibilityVersion>,
    /// Target number for proof-of-work
    pow_target: f64,
}

impl LocalPeerInfo {
    pub fn new(
        listener_port: u16,
        identity: Arc<Identity>,
        version: Arc<ShellCompatibilityVersion>,
        pow_target: f64,
    ) -> Self {
        LocalPeerInfo {
            listener_port,
            identity,
            version,
            pow_target,
        }
    }

    pub fn listener_port(&self) -> u16 {
        self.listener_port
    }
}
