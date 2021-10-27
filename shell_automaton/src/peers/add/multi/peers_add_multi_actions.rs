// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Add multiple peers as potential peers.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersAddMultiAction {
    pub addresses: Vec<SocketAddr>,
}
