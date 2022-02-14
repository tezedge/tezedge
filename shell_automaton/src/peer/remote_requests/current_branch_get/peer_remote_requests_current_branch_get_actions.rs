// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;

use crate::request::RequestId;
use crate::service::storage_service::StorageError;
use crate::{EnablingCondition, State};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetInitAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map(|p| &p.remote_requests.current_branch_get)
            .filter(|v| !v.is_pending())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetPendingAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.current_branch_get.is_init())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetNextBlockInitAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetNextBlockInitAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| {
                p.remote_requests
                    .current_branch_get
                    .next_block_is_idle_or_success()
                    && !p.remote_requests.current_branch_get.is_complete()
            })
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetNextBlockPendingAction {
    pub address: SocketAddr,
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetNextBlockPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .and_then(|p| p.remote_requests.current_branch_get.next_block())
            .filter(|b| b.is_init())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetNextBlockErrorAction {
    pub address: SocketAddr,
    pub error: StorageError,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetNextBlockErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .and_then(|p| p.remote_requests.current_branch_get.next_block())
            .filter(|b| b.is_pending())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetNextBlockSuccessAction {
    pub address: SocketAddr,
    pub result: Option<BlockHash>,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetNextBlockSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .and_then(|p| p.remote_requests.current_branch_get.next_block())
            .filter(|b| b.is_pending())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetSuccessAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.current_branch_get.is_complete())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsCurrentBranchGetFinishAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsCurrentBranchGetFinishAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.current_branch_get.is_success())
            .is_some()
    }
}
