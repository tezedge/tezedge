// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::{request::RequestId, service::storage_service::StorageError, EnablingCondition, State};

use super::MAX_PEER_REMOTE_BLOCK_HEADER_REQUESTS;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockHeaderGetEnqueueAction {
    pub address: SocketAddr,
    pub block_hash: BlockHash,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockHeaderGetEnqueueAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| {
                !p.remote_requests
                    .block_header_get
                    .queue
                    .contains(&self.block_hash)
                    && p.remote_requests.block_header_get.queue.len()
                        < MAX_PEER_REMOTE_BLOCK_HEADER_REQUESTS
            })
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockHeaderGetInitNextAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockHeaderGetInitNextAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map(|p| &p.remote_requests.block_header_get)
            .filter(|v| !v.current.is_pending() && !v.queue.is_empty())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockHeaderGetPendingAction {
    pub address: SocketAddr,
    pub block_hash: BlockHash,
    pub storage_req_id: RequestId,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockHeaderGetPendingAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .map(|p| &p.remote_requests.block_header_get)
            .and_then(|v| v.queue.iter().nth(0).filter(|b| *b == &self.block_hash))
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockHeaderGetErrorAction {
    pub address: SocketAddr,
    pub error: StorageError,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockHeaderGetErrorAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.block_header_get.current.is_pending())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockHeaderGetSuccessAction {
    pub address: SocketAddr,
    pub result: Option<BlockHeader>,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockHeaderGetSuccessAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.block_header_get.current.is_pending())
            .is_some()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerRemoteRequestsBlockHeaderGetFinishAction {
    pub address: SocketAddr,
}

impl EnablingCondition<State> for PeerRemoteRequestsBlockHeaderGetFinishAction {
    fn is_enabled(&self, state: &State) -> bool {
        state
            .peers
            .get_handshaked(&self.address)
            .filter(|p| p.remote_requests.block_header_get.current.is_success())
            .is_some()
    }
}
