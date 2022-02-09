// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::action::BootstrapNewCurrentHeadAction;
use crate::block_applier::BlockApplierEnqueueBlockAction;
use crate::peer::message::write::PeerMessageWriteInitAction;
use crate::peers::graylist::PeersGraylistAddressAction;
use crate::peers::init::PeersInitAction;
use crate::service::actors_service::ActorsMessageFrom;
use crate::service::{ActorsService, Service};
use crate::shutdown::ShutdownInitAction;
use crate::{Action, ActionWithMeta, Store};

pub fn actors_effects<S: Service>(store: &mut Store<S>, action: &ActionWithMeta) {
    match &action.action {
        Action::WakeupEvent(_) => {
            while let Ok(msg) = store.service.actors().try_recv() {
                match msg {
                    ActorsMessageFrom::Shutdown => {
                        store.dispatch(ShutdownInitAction {});
                    }
                    ActorsMessageFrom::P2pInit => {
                        store.dispatch(PeersInitAction {});
                    }
                    ActorsMessageFrom::PeerStalled(peer_id) => {
                        // store.dispatch(PeersGraylistAddressAction {
                        //     address: peer_id.address,
                        // });
                    }
                    ActorsMessageFrom::BlacklistPeer(peer_id, _) => {
                        // store.dispatch(PeersGraylistAddressAction {
                        //     address: peer_id.address,
                        // });
                    }
                    ActorsMessageFrom::SendMessage(peer_id, message) => {
                        store.dispatch(PeerMessageWriteInitAction {
                            address: peer_id.address,
                            message,
                        });
                    }
                    ActorsMessageFrom::NewCurrentHead {
                        chain_id,
                        block,
                        is_bootstrapped,
                    } => {
                        store.dispatch(BootstrapNewCurrentHeadAction {
                            chain_id,
                            block,
                            is_bootstrapped,
                        });
                    }
                    ActorsMessageFrom::ApplyBlock {
                        chain_id,
                        block_hash,
                        callback,
                    } => {
                        // store
                        //     .service
                        //     .actors()
                        //     .register_apply_block_callback(block_hash.clone(), callback);
                        // store.dispatch(BlockApplierEnqueueBlockAction {
                        //     chain_id,
                        //     block_hash,
                        // });
                    }
                }
            }
        }
        _ => {}
    }
}
