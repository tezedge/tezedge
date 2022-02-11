// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use storage::OperationKey;
use tezos_messages::p2p::encoding::operations_for_blocks::OperationsForBlocksMessage;

use crate::request::RequestId;
use crate::service::storage_service::StorageError;
use crate::{EnablingCondition, State};

use super::MAX_PEER_REMOTE_BLOCK_OPERATIONS_REQUESTS;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockOperationsGetEnqueueAction {
    pub address: SocketAddr,
    pub key: OperationKey,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockOperationsGetEnqueueAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get(&self.address)
            .and_then(|p| p.status.as_handshaked())
            .filter(|p| {
                p.remote_requests
                    .block_operations_get
                    .queue
                    .contains(&self.key)
                    && p.remote_requests.block_operations_get.queue.len()
                        < MAX_PEER_REMOTE_BLOCK_OPERATIONS_REQUESTS
            })
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockOperationsGetInitNextAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockOperationsGetInitNextAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get(&self.address)
            .and_then(|p| p.status.as_handshaked())
            .map(|p| &p.remote_requests.block_operations_get)
            .filter(|v| !v.current.is_pending() && !v.queue.is_empty())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockOperationsGetPendingAction {
    pub address: SocketAddr,
    pub key: OperationKey,
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockOperationsGetPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map(|p| &p.remote_requests.block_operations_get)
            .and_then(|v| v.queue.iter().nth(0).filter(|b| *b == &self.key))
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockOperationsGetErrorAction {
    pub address: SocketAddr,
    pub error: StorageError,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockOperationsGetErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.block_operations_get.current.is_pending())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockOperationsGetSuccessAction {
    pub address: SocketAddr,
    pub result: Option<OperationsForBlocksMessage>,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockOperationsGetSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.block_operations_get.current.is_pending())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockOperationsGetFinishAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockOperationsGetFinishAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.block_operations_get.current.is_success())
            .is_some()
    }
}
