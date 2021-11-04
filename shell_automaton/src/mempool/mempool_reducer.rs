// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::MessageHash;

use crate::{State, Action, ActionWithId};

use super::{
    MempoolGetOperationsPendingAction, MempoolRecvDoneAction, MempoolOperationRecvDoneAction,
    MempoolBroadcastDoneAction, MempoolOperationInjectAction,
};

pub fn mempool_reducer(state: &mut State, action: &ActionWithId<Action>) {
    let mut mempool_state = &mut state.mempool;

    match &action.action {
        Action::MempoolRecvDone(MempoolRecvDoneAction { address, head_state, message }) => {
            let pending = message.pending().iter().cloned();
            let known_valid = message.known_valid().iter().cloned();

            // TODO: check whether we can accept this head
            mempool_state.head_state = Some(head_state.clone());

            let peer = mempool_state.peer_state.entry(*address).or_default();
            for hash in pending.chain(known_valid) {
                let known = mempool_state.pending_operations.contains_key(&hash)
                    || mempool_state.applied_operations.contains_key(&hash)
                    || mempool_state.branch_delayed_operations.contains_key(&hash)
                    || mempool_state.branch_refused_operations.contains_key(&hash)
                    || mempool_state.refused_operations.contains(&hash);
                if !known {
                    peer.requesting_full_content.insert(hash.clone());
                    // of course peer knows about it, because he sent us it
                    peer.known_operations.insert(hash);
                }
            }
        },
        Action::MempoolGetOperationsPending(MempoolGetOperationsPendingAction { address }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();
            peer.pending_full_content.extend(peer.requesting_full_content.drain());
        },
        Action::MempoolOperationRecvDone(MempoolOperationRecvDoneAction { address, operation }) => {
            let operation_hash = match operation.message_typed_hash() {
                Ok(v) => v,
                Err(err) => {
                    // TODO: peer send bad operation, should log the error,
                    // maybe should disconnect the peer
                    let _ = err;
                    return;
                }
            };
            let peer = mempool_state.peer_state.entry(*address).or_default();

            if !peer.pending_full_content.remove(&operation_hash) {
                // TODO: received operation, but we did not requested it
            }

            // TODO: prevalidate the operation
            mempool_state.applied_operations.insert(operation_hash, operation.clone());
        },
        Action::MempoolOperationInject(MempoolOperationInjectAction { operation, operation_hash, .. }) => {
            // TODO: prevalidate the operation
            mempool_state.applied_operations.insert(operation_hash.clone(), operation.clone());
        },
        Action::MempoolBroadcastDone(MempoolBroadcastDoneAction { address, known_valid, pending }) => {
            let peer = mempool_state.peer_state.entry(*address).or_default();

            peer.known_operations.extend(known_valid.iter().cloned());
            peer.known_operations.extend(pending.iter().cloned());
        },
        _ => (),
    }
}
