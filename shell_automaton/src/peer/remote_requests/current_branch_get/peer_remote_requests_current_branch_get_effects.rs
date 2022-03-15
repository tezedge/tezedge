// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use tezos_messages::p2p::encoding::peer::PeerMessage;
use tezos_messages::p2p::encoding::prelude::CurrentBranchMessage;

use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::service::storage_service::StorageRequestPayload;
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    PeerRemoteRequestsCurrentBranchGetFinishAction,
    PeerRemoteRequestsCurrentBranchGetNextBlockInitAction,
    PeerRemoteRequestsCurrentBranchGetNextBlockPendingAction,
    PeerRemoteRequestsCurrentBranchGetPendingAction, PeerRemoteRequestsCurrentBranchGetState,
    PeerRemoteRequestsCurrentBranchGetSuccessAction,
};

pub fn peer_remote_requests_current_branch_get_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    match &action.action {
        Action::PeerRemoteRequestsCurrentBranchGetInit(content) => {
            store.dispatch(PeerRemoteRequestsCurrentBranchGetPendingAction {
                address: content.address,
            });
        }
        Action::PeerRemoteRequestsCurrentBranchGetPending(content) => {
            if !store.dispatch(PeerRemoteRequestsCurrentBranchGetNextBlockInitAction {
                address: content.address,
            }) {
                store.dispatch(PeerRemoteRequestsCurrentBranchGetSuccessAction {
                    address: content.address,
                });
            }
        }
        Action::PeerRemoteRequestsCurrentBranchGetNextBlockInit(content) => {
            let peer = match store.state().peers.get_handshaked(&content.address) {
                Some(v) => v,
                None => return,
            };

            let level = match peer
                .remote_requests
                .current_branch_get
                .next_block()
                .and_then(|v| v.level())
            {
                Some(v) => v,
                None => return,
            };
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockHashByLevelGet(level),
                requestor: StorageRequestor::Peer(content.address),
            });
            let storage_req_id = store.state().storage.requests.last_added_req_id();
            store.dispatch(PeerRemoteRequestsCurrentBranchGetNextBlockPendingAction {
                address: content.address,
                storage_req_id,
            });
        }
        Action::PeerRemoteRequestsCurrentBranchGetNextBlockError(content) => {
            // TODO: log error
            store.dispatch(PeerRemoteRequestsCurrentBranchGetSuccessAction {
                address: content.address,
            });
        }
        Action::PeerRemoteRequestsCurrentBranchGetNextBlockSuccess(content) => {
            if !store.dispatch(PeerRemoteRequestsCurrentBranchGetNextBlockInitAction {
                address: content.address,
            }) {
                store.dispatch(PeerRemoteRequestsCurrentBranchGetSuccessAction {
                    address: content.address,
                });
            }
        }
        Action::PeerRemoteRequestsCurrentBranchGetSuccess(content) => {
            let peer = match store.state().peers.get_handshaked(&content.address) {
                Some(v) => v,
                None => return,
            };
            let chain_id = store.state().config.chain_id.clone();
            let current_branch = match &peer.remote_requests.current_branch_get {
                PeerRemoteRequestsCurrentBranchGetState::Success { result } => result.clone(),
                _ => return,
            };
            let message =
                PeerMessage::CurrentBranch(CurrentBranchMessage::new(chain_id, current_branch));
            store.dispatch(PeerMessageWriteInitAction {
                address: content.address,
                message: Arc::new(message.into()),
            });
            store.dispatch(PeerRemoteRequestsCurrentBranchGetFinishAction {
                address: content.address,
            });
        }
        _ => {}
    }
}
