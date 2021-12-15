// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::mempool::BlockAppliedAction;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::service::actors_service::ActorsMessageFrom;
use crate::service::{ActorsService, Service};
use crate::{Action, ActionWithMeta, Store};

pub fn actors_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok(msg) = store.service.actors().try_recv() {
                match msg {
                    ActorsMessageFrom::Shutdown => {
                        // TODO
                    }
                    ActorsMessageFrom::PeerStalled(peer_id) => {
                        store.dispatch(PeersGraylistAddressAction {
                            address: peer_id.address,
                        });
                    }
                    ActorsMessageFrom::BlacklistPeer(peer_id, _) => {
                        store.dispatch(PeersGraylistAddressAction {
                            address: peer_id.address,
                        });
                    }
                    ActorsMessageFrom::SendMessage(peer_id, message) => {
                        store.dispatch(PeerMessageWriteInitAction {
                            address: peer_id.address,
                            message,
                        });
                    }
                    ActorsMessageFrom::BlockApplied(chain_id, block, hash, block_metadata_hash, ops_metadata_hash, is_bootstrapped) => {
                        store
                            .dispatch(
                                BlockAppliedAction {
                                    chain_id,
                                    block,
                                    hash,
                                    block_metadata_hash,
                                    ops_metadata_hash,
                                    is_bootstrapped,
                                },
                            );
                    }
                }
            }
        }
        _ => {}
    }
}
