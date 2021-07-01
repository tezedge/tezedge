// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, RwLock};

use riker::actors::*;
use slog::{info, Logger};

use crypto::hash::ChainId;
use storage::PersistentStorage;
use tezos_wrapper::TezosApiConnectionPool;

use crate::mempool::mempool_prevalidator::{MempoolPrevalidator, MempoolPrevalidatorBasicRef};
use crate::mempool::mempool_state::MempoolState;
use crate::shell_channel::ShellChannelRef;
use crate::state::StateError;

pub mod mempool_prevalidator;
pub mod mempool_state;

/// In-memory synchronized struct for sharing between threads/actors
pub type CurrentMempoolStateStorageRef = Arc<RwLock<MempoolState>>;

/// Inits empty mempool state storage
pub fn init_mempool_state_storage() -> CurrentMempoolStateStorageRef {
    Arc::new(RwLock::new(MempoolState::default()))
}

pub fn find_mempool_prevalidator(sys: &ActorSystem, chain_id: &ChainId) -> Option<BasicActorRef> {
    let expected_prevalidator_name = MempoolPrevalidator::name(chain_id);
    sys.user_root()
        .children()
        .find(|actor_ref| expected_prevalidator_name.eq(actor_ref.name()))
}

pub struct MempoolPrevalidatorFactory {
    shell_channel: ShellChannelRef,
    persistent_storage: PersistentStorage,
    current_mempool_state: CurrentMempoolStateStorageRef,
    tezos_readonly_mempool_api: Arc<TezosApiConnectionPool>,
    /// Indicates if mempool is disabled to propagate to p2p
    pub p2p_disable_mempool: bool,
}

impl MempoolPrevalidatorFactory {
    pub fn new(
        shell_channel: ShellChannelRef,
        persistent_storage: PersistentStorage,
        current_mempool_state: CurrentMempoolStateStorageRef,
        tezos_readonly_mempool_api: Arc<TezosApiConnectionPool>,
        p2p_disable_mempool: bool,
    ) -> Self {
        Self {
            shell_channel,
            persistent_storage,
            current_mempool_state,
            tezos_readonly_mempool_api,
            p2p_disable_mempool,
        }
    }

    pub fn get_or_start_mempool(
        &self,
        chain_id: ChainId,
        sys: &ActorSystem,
        log: &Logger,
    ) -> Result<Option<MempoolPrevalidatorBasicRef>, StateError> {
        if self.p2p_disable_mempool {
            info!(log, "Mempool is disabled, so do not start one");
            Ok(None)
        } else {
            // check if exists any
            if let Some(existing_mempool_prevalidator) = find_mempool_prevalidator(sys, &chain_id) {
                info!(log, "Found already started mempool prevalidator"; "chain_id" => chain_id.to_base58_check());
                return Ok(Some(existing_mempool_prevalidator));
            }

            // find if not found start new one
            info!(log, "Starting mempool prevalidator"; "chain_id" => chain_id.to_base58_check());
            MempoolPrevalidator::actor(
                &sys,
                self.shell_channel.clone(),
                self.persistent_storage.clone(),
                self.current_mempool_state.clone(),
                chain_id,
                self.tezos_readonly_mempool_api.clone(),
                log.clone(),
            )
            .map_err(|e| StateError::ProcessingError {
                reason: format!("Failed to start mempool prevalidator, reason: {}", e),
            })
            .map(|mp| Some(MempoolPrevalidatorBasicRef::from(mp)))
        }
    }
}
