// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezos_messages::p2p::binary_message::MessageHash;

use crate::{State, Action, ActionWithMeta};
use crate::protocol::ProtocolAction;

use super::{
    MempoolGetOperationsPendingAction, MempoolRecvDoneAction, MempoolOperationRecvDoneAction,
    MempoolBroadcastDoneAction, MempoolOperationInjectAction, BlockAppliedAction,
    HeadState,
};

pub fn mempool_reducer(state: &mut State, action: &ActionWithMeta) {
    let mut mempool_state = &mut state.mempool;

    match &action.action {
        Action::Protocol(act) => {
            match act {
                ProtocolAction::PrevalidatorForMempoolReady(prevalidator) => {
                    mempool_state.prevalidator = Some(prevalidator.clone());
                    println!("prevalidator: {:?}", prevalidator);
                    // mempool_state.prevalidator_block = Some(...);
                },
                act => {
                    println!("{:?}", act);
                },
            }
        },
        Action::BlockApplied(BlockAppliedAction { chain_id, block }) => {
            mempool_state.local_head_state = Some(HeadState {
                chain_id: chain_id.clone(),
                current_block: block.clone(),
                // TODO(vlad): unwrap
                current_block_hash: block.message_typed_hash().unwrap(),
            });
            match block.message_typed_hash() {
                Ok(hash) => drop(mempool_state.applied_block.insert(hash)),
                Err(err) => {
                    // TODO(vlad): unwrap
                    let _ = err;
                },
            }
        }
        Action::MempoolRecvDone(MempoolRecvDoneAction { address, head_state, message }) => {
            let pending = message.pending().iter().cloned();
            let known_valid = message.known_valid().iter().cloned();

            let peer = mempool_state.peer_state.entry(*address).or_default();
            // TODO(vlad): check whether we can accept this head
            peer.head_state = Some(head_state.clone());
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
                    // TODO(vlad): peer send bad operation, should log the error,
                    // maybe should disconnect the peer
                    let _ = err;
                    return;
                }
            };
            let peer = mempool_state.peer_state.entry(*address).or_default();

            if !peer.pending_full_content.remove(&operation_hash) {
                // TODO(vlad): received operation, but we did not requested it
            }

            // TODO(vlad): prevalidate the operation
            mempool_state.applied_operations.insert(operation_hash, operation.clone());
        },
        Action::MempoolOperationInject(MempoolOperationInjectAction { operation, operation_hash, .. }) => {
            // TODO(vlad): prevalidate the operation
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
