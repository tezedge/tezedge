// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use super::block_header_get::PeerRemoteRequestsBlockHeaderGetState;
use super::block_operations_get::PeerRemoteRequestsBlockOperationsGetState;
use super::current_branch_get::PeerRemoteRequestsCurrentBranchGetState;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct PeerRemoteRequestsState {
    pub block_header_get: PeerRemoteRequestsBlockHeaderGetState,
    pub block_operations_get: PeerRemoteRequestsBlockOperationsGetState,
    pub current_branch_get: PeerRemoteRequestsCurrentBranchGetState,
}
