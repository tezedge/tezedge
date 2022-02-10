// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::service::storage_service::StorageRequestPayload;
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    PeerRemoteRequestsBlockHeaderGetFinishAction, PeerRemoteRequestsBlockHeaderGetInitNextAction,
    PeerRemoteRequestsBlockHeaderGetPendingAction,
};

pub fn peer_remote_requests_block_header_get_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    match &action.action {
        Action::PeerRemoteRequestsBlockHeaderGetEnqueue(content) => {
            store.dispatch(PeerRemoteRequestsBlockHeaderGetInitNextAction {
                address: content.address,
            });
        }
        Action::PeerRemoteRequestsBlockHeaderGetInitNext(content) => {
            let peer = match store.state().peers.get_handshaked(&content.address) {
                Some(v) => v,
                None => return,
            };
            let block_hash = match peer.remote_requests.block_header_get.queue.iter().nth(0) {
                Some(v) => v.clone(),
                None => return,
            };
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockHeaderGet(block_hash.clone()),
                requestor: StorageRequestor::Peer(content.address),
            });
            let storage_req_id = store.state().storage.requests.last_added_req_id();
            store.dispatch(PeerRemoteRequestsBlockHeaderGetPendingAction {
                address: content.address,
                block_hash,
                storage_req_id,
            });
        }
        Action::PeerRemoteRequestsBlockHeaderGetError(content) => {
            store.dispatch(PeerRemoteRequestsBlockHeaderGetInitNextAction {
                address: content.address,
            });
        }
        Action::PeerRemoteRequestsBlockHeaderGetSuccess(content) => {
            if let Some(header) = content.block_header.clone() {
                let message = PeerMessage::BlockHeader(header.into());
                store.dispatch(PeerMessageWriteInitAction {
                    address: content.address,
                    message: Arc::new(message.into()),
                });
            } else {
                // TODO: log that block header wasn't found.
            }
            if !store.dispatch(PeerRemoteRequestsBlockHeaderGetInitNextAction {
                address: content.address,
            }) {
                store.dispatch(PeerRemoteRequestsBlockHeaderGetFinishAction {
                    address: content.address,
                });
            }
        }
        _ => {}
    }
}
