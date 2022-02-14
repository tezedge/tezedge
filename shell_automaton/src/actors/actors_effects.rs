// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::action::BootstrapNewCurrentHeadAction;
use crate::block_applier::BlockApplierEnqueueBlockAction;
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
                    ActorsMessageFrom::ApplyBlock {
                        block_hash,
                        callback,
                        ..
                    } => {
                        store
                            .service
                            .actors()
                            .register_apply_block_callback(block_hash.clone(), callback);
                        store.dispatch(BlockApplierEnqueueBlockAction {
                            block_hash,
                            injector_rpc_id: None,
                        });
                    }
                }
            }
        }
        _ => {}
    }
}
