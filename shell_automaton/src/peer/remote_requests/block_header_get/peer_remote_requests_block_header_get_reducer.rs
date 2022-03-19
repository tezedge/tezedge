// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::{Action, ActionWithMeta, State};

use super::PeerRemoteRequestsBlockHeaderGetCurrentState;

pub fn peer_remote_requests_block_header_get_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerRemoteRequestsBlockHeaderGetEnqueue(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };

            peer.remote_requests
                .block_header_get
                .queue
                .insert(content.block_hash.clone());
        }
        Action::PeerRemoteRequestsBlockHeaderGetPending(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };

            let queue = &mut peer.remote_requests.block_header_get.queue;
            let next_block = queue.iter().next().cloned();
            let block_hash = next_block
                .filter(|nb| nb == &content.block_hash)
                .map(|key| {
                    queue.remove(&key);
                    key
                })
                .unwrap_or_else(|| content.block_hash.clone());
            peer.remote_requests.block_header_get.current =
                PeerRemoteRequestsBlockHeaderGetCurrentState::Pending {
                    block_hash,
                    storage_req_id: content.storage_req_id,
                };
        }
        Action::PeerRemoteRequestsBlockHeaderGetError(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current = &mut peer.remote_requests.block_header_get.current;
            let block_hash = match current.block_hash() {
                Some(v) => v.clone(),
                None => return,
            };
            *current = PeerRemoteRequestsBlockHeaderGetCurrentState::Error {
                block_hash,
                error: content.error.clone(),
            };
        }
        Action::PeerRemoteRequestsBlockHeaderGetSuccess(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current = &mut peer.remote_requests.block_header_get.current;
            let block_hash = match current.block_hash() {
                Some(v) => v.clone(),
                None => return,
            };
            *current = PeerRemoteRequestsBlockHeaderGetCurrentState::Success {
                block_hash,
                result: content.result.clone(),
            };
        }
        Action::PeerRemoteRequestsBlockHeaderGetFinish(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current = &mut peer.remote_requests.block_header_get.current;
            *current = PeerRemoteRequestsBlockHeaderGetCurrentState::Idle {
                time: action.time_as_nanos(),
            };
        }
        _ => {}
    }
}
