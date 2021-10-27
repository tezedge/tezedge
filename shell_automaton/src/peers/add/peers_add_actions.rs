// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

use crate::peer::PeerToken;

/// Add incoming peer to peers.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeersAddIncomingPeerAction {
    pub token: PeerToken,
    pub address: SocketAddr,
}
