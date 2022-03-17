// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::BlockHash;
use tezos_messages::p2p::{binary_message::MessageHash, encoding::peer::PeerMessage};

use crate::{
    current_head_precheck::CurrentHeadPrecheckSuccessAction,
    mempool::mempool_actions::BlockInjectAction,
    peer::{
        message::write::{
            PeerMessageWriteErrorAction, PeerMessageWriteInitAction, PeerMessageWriteSuccessAction,
        },
        Peer,
    },
    Action,
};

use super::{stats_current_head_actions::*, PendingMessage};

// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub fn stats_current_head_effects<S>(store: &mut crate::Store<S>, action: &crate::ActionWithMeta)
where
    S: crate::Service,
{
    match &action.action {
        Action::BlockInject(BlockInjectAction {
            chain_id: _,
            block_hash,
            block_header,
            injected_timestamp,
        }) => {
            let node_id = store.state.get().config.identity.peer_id.clone();
            store.service.statistics().map(|s| {
                s.block_new(
                    block_hash.clone(),
                    block_header.level(),
                    block_header.timestamp().into(),
                    block_header.validation_pass(),
                    action.time_as_nanos(),
                    None,
                    Some(node_id),
                    Some(*injected_timestamp),
                );
            });
        }
        Action::StatsCurrentHeadPrecheckInit(StatsCurrentHeadPrecheckInitAction { hash }) => {
            store.service.statistics().map(|s| {
                s.block_precheck_start(hash, action.time_as_nanos());
            });
        }
        Action::CurrentHeadPrecheckSuccess(CurrentHeadPrecheckSuccessAction {
            block_hash,
            baker,
            priority,
            ..
        }) => {
            store.service.statistics().map(|s| {
                s.block_precheck_end(block_hash, baker.clone(), *priority, action.time_as_nanos());
            });
        }
        Action::PeerMessageWriteInit(PeerMessageWriteInitAction { message, address }) => {
            match message.message() {
                PeerMessage::CurrentHead(current_head)
                    if current_head.current_mempool().is_empty() =>
                {
                    let current_block_header = current_head.current_block_header();
                    let block_hash = match current_block_header.message_typed_hash::<BlockHash>() {
                        Ok(v) => v,
                        Err(_) => return,
                    };

                    let node_id = store
                        .state
                        .get()
                        .peers
                        .get(&address)
                        .and_then(Peer::public_key_hash);
                    store.service.statistics().map(|s| {
                        s.block_send_start(&block_hash, *address, node_id, action.time_as_nanos());
                    });

                    store.dispatch(StatsCurrentHeadPrepareSendAction {
                        address: *address,
                        message: PendingMessage::CurrentHead { block_hash },
                    });
                }
                PeerMessage::OperationsForBlocks(ops_for_blocks) => {
                    let node_id = store
                        .state
                        .get()
                        .peers
                        .get(&address)
                        .and_then(Peer::public_key_hash);
                    store.service.statistics().map(|s| {
                        s.block_operations_send_start(
                            ops_for_blocks.operations_for_block().block_hash(),
                            action.time_as_nanos(),
                            *address,
                            node_id,
                            ops_for_blocks.operations_for_block().validation_pass(),
                        );
                    });
                    store.dispatch(StatsCurrentHeadPrepareSendAction {
                        address: *address,
                        message: PendingMessage::OperationsForBlocks {
                            block_hash: ops_for_blocks.operations_for_block().block_hash().clone(),
                            validation_pass: ops_for_blocks
                                .operations_for_block()
                                .validation_pass(),
                        },
                    });
                }
                _ => (),
            }
        }
        Action::PeerMessageWriteSuccess(PeerMessageWriteSuccessAction { address }) => {
            match store
                .state
                .get()
                .stats
                .current_head
                .pending_messages
                .get(address)
            {
                Some(PendingMessage::CurrentHead { block_hash }) => {
                    let node_id = store
                        .state
                        .get()
                        .peers
                        .get(&address)
                        .and_then(Peer::public_key_hash);
                    store.service.statistics().map(|s| {
                        s.block_send_end(block_hash, *address, node_id, action.time_as_nanos());
                    });
                }
                Some(PendingMessage::OperationsForBlocks {
                    block_hash,
                    validation_pass,
                }) => {
                    let node_id = store
                        .state
                        .get()
                        .peers
                        .get(&address)
                        .and_then(Peer::public_key_hash);
                    store.service.statistics().map(|s| {
                        s.block_operations_send_end(
                            block_hash,
                            action.time_as_nanos(),
                            *address,
                            node_id,
                            *validation_pass,
                        );
                    });
                }
                _ => (),
            }
            store.dispatch(StatsCurrentHeadSentAction { address: *address });
        }
        Action::PeerMessageWriteError(PeerMessageWriteErrorAction { address, .. }) => {
            store.dispatch(StatsCurrentHeadSentErrorAction { address: *address });
        }
        _ => (),
    }
}
