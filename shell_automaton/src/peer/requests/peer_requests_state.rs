// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::potential_peers_get::PeerRequestsPotentialPeersGetState;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PeerRequestsState {
    pub potential_peers_get: PeerRequestsPotentialPeersGetState,
}
