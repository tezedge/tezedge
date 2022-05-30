// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::seeded_step::{Seed, Step};
use tezos_messages::p2p::encoding::prelude::CurrentBranch;

use crate::{Action, ActionWithMeta, State};

use super::PeerRemoteRequestsCurrentBranchGetState;

pub fn peer_remote_requests_current_branch_get_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::PeerRemoteRequestsCurrentBranchGetInit(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };

            peer.remote_requests.current_branch_get =
                PeerRemoteRequestsCurrentBranchGetState::Init {
                    time: action.time_as_nanos(),
                };
        }
        Action::PeerRemoteRequestsCurrentBranchGetPending(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current_head = match state.current_head.get() {
                Some(v) => v,
                None => return,
            };
            let seed = Seed::new(&state.config.identity.peer_id, &peer.public_key_hash);
            let step = Step::init(&seed, &current_head.hash);

            peer.remote_requests.current_branch_get =
                PeerRemoteRequestsCurrentBranchGetState::Pending {
                    current_head: (*current_head.header).clone(),
                    history: vec![],
                    step,
                    next_block: Default::default(),
                };
        }
        Action::PeerRemoteRequestsCurrentBranchGetNextBlockInit(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let get_state = &mut peer.remote_requests.current_branch_get;
            let next_level = match get_state.step_next_level() {
                Some(v) => v,
                None => return,
            };
            if let Some(nb) = get_state.next_block_mut() {
                nb.init(next_level)
            }
        }
        Action::PeerRemoteRequestsCurrentBranchGetNextBlockPending(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let next_block_state = peer.remote_requests.current_branch_get.next_block_mut();
            if let Some(nb) = next_block_state {
                nb.to_pending(content.storage_req_id)
            }
        }
        Action::PeerRemoteRequestsCurrentBranchGetNextBlockError(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let next_block_state = peer.remote_requests.current_branch_get.next_block_mut();
            if let Some(nb) = next_block_state {
                nb.to_error(content.error.clone())
            }
        }
        Action::PeerRemoteRequestsCurrentBranchGetNextBlockSuccess(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            if let PeerRemoteRequestsCurrentBranchGetState::Pending {
                history,
                next_block,
                ..
            } = &mut peer.remote_requests.current_branch_get
            {
                next_block.to_success(content.result.clone());
                if let Some(block_hash) = content.result.clone() {
                    history.push(block_hash);
                }
            }
        }
        Action::PeerRemoteRequestsCurrentBranchGetSuccess(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            let current_branch = match &peer.remote_requests.current_branch_get {
                PeerRemoteRequestsCurrentBranchGetState::Pending {
                    current_head,
                    history,
                    ..
                } => CurrentBranch::new(current_head.clone(), history.clone()),
                _ => return,
            };

            peer.remote_requests.current_branch_get =
                PeerRemoteRequestsCurrentBranchGetState::Success {
                    result: current_branch,
                };
        }
        Action::PeerRemoteRequestsCurrentBranchGetFinish(content) => {
            let peer = match state.peers.get_handshaked_mut(&content.address) {
                Some(v) => v,
                None => return,
            };
            peer.remote_requests.current_branch_get =
                PeerRemoteRequestsCurrentBranchGetState::Idle {
                    time: action.time_as_nanos(),
                };
        }
        _ => {}
    }
}
