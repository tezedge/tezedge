// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::block_header_get::PeerRemoteRequestsBlockHeaderGetState;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PeerRemoteRequestsState {
    pub block_header_get: PeerRemoteRequestsBlockHeaderGetState,
}
