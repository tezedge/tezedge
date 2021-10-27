// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

/// Peer closed connection with us.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerConnectionClosedAction {
    pub address: SocketAddr,
}
