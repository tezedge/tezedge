// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use tezos_messages::p2p::encoding::peer::PeerMessage;

use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::service::storage_service::StorageRequestPayload;
use crate::storage::request::{StorageRequestCreateAction, StorageRequestor};
use crate::{Action, ActionWithMeta, Service, Store};

use super::{
    PeerRemoteRequestsBlockOperationsGetFinishAction,
    PeerRemoteRequestsBlockOperationsGetInitNextAction,
    PeerRemoteRequestsBlockOperationsGetPendingAction,
};

pub fn peer_remote_requests_block_operations_get_effects<S>(
    store: &mut Store<S>,
    action: &ActionWithMeta,
) where
    S: Service,
{
    match &action.action {
        Action::PeerRemoteRequestsBlockOperationsGetEnqueue(content) => {
            store.dispatch(PeerRemoteRequestsBlockOperationsGetInitNextAction {
                address: content.address,
            });
        }
        Action::PeerRemoteRequestsBlockOperationsGetInitNext(content) => {
            let peer = match store.state().peers.get_handshaked(&content.address) {
                Some(v) => v,
                None => return,
            };
            let key = match peer
                .remote_requests
                .block_operations_get
                .queue
                .iter()
                .next()
            {
                Some(v) => v.clone(),
                None => return,
            };
            let storage_req_id = store.state().storage.requests.next_req_id();
            store.dispatch(StorageRequestCreateAction {
                payload: StorageRequestPayload::BlockOperationsGet(key.clone()),
                requestor: StorageRequestor::Peer(content.address),
            });
            store.dispatch(PeerRemoteRequestsBlockOperationsGetPendingAction {
                address: content.address,
                key,
                storage_req_id,
            });
        }
        Action::PeerRemoteRequestsBlockOperationsGetError(content) => {
            store.dispatch(PeerRemoteRequestsBlockOperationsGetInitNextAction {
                address: content.address,
            });
        }
        Action::PeerRemoteRequestsBlockOperationsGetSuccess(content) => {
            if let Some(operations) = content.result.clone() {
                let message = PeerMessage::OperationsForBlocks(operations);
                store.dispatch(PeerMessageWriteInitAction {
                    address: content.address,
                    message: Arc::new(message.into()),
                });
            } else {
                // TODO: log that block operations weren't found.
            }
            if !store.dispatch(PeerRemoteRequestsBlockOperationsGetInitNextAction {
                address: content.address,
            }) {
                store.dispatch(PeerRemoteRequestsBlockOperationsGetFinishAction {
                    address: content.address,
                });
            }
        }
        _ => {}
    }
}
