// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::PeerRemoteRequestsBlockOperationsGetCurrentState;

pub fn peer_remote_requests_block_operations_get_reducer(
    state: &mut State,
    action: &ActionWithMeta,
) {
    match &action.action {
        Action::PeerRemoteRequestsBlockOperationsGetEnqueue(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };

            peer.remote_requests
                .block_operations_get
                .queue
                .insert(content.key.clone());
        }
        Action::PeerRemoteRequestsBlockOperationsGetPending(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };

            let queue = &mut peer.remote_requests.block_operations_get.queue;
            let next_key = queue.iter().next().cloned();
            let key = next_key
                .filter(|nb| nb == &content.key)
                .map(|key| {
                    queue.remove(&key);
                    key
                })
                .unwrap_or_else(|| content.key.clone());
            peer.remote_requests.block_operations_get.current =
                PeerRemoteRequestsBlockOperationsGetCurrentState::Pending {
                    key,
                    storage_req_id: content.storage_req_id,
                };
        }
        Action::PeerRemoteRequestsBlockOperationsGetError(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current = &mut peer.remote_requests.block_operations_get.current;
            let key = match current.key() {
                Some(v) => v.clone(),
                None => return,
            };
            *current = PeerRemoteRequestsBlockOperationsGetCurrentState::Error {
                key,
                error: content.error.clone(),
            };
        }
        Action::PeerRemoteRequestsBlockOperationsGetSuccess(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current = &mut peer.remote_requests.block_operations_get.current;
            let key = match current.key() {
                Some(v) => v.clone(),
                None => return,
            };
            *current = PeerRemoteRequestsBlockOperationsGetCurrentState::Success {
                key,
                result: content.result.clone(),
            };
        }
        Action::PeerRemoteRequestsBlockOperationsGetFinish(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current = &mut peer.remote_requests.block_operations_get.current;
            *current = PeerRemoteRequestsBlockOperationsGetCurrentState::Idle {
                time: action.time_as_nanos(),
            };
        }
        _ => {}
    }
}
