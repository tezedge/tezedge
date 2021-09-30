// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use chrono::SecondsFormat;
use riker::actors::*;
use slog::{info, warn, Logger};
use thiserror::Error;

use crypto::hash::ChainId;
use shell_integration::*;
use storage::PersistentStorage;
use tezos_wrapper::TezosApiConnectionPool;

use crate::chain_manager::ChainManagerRef;
use crate::mempool::mempool_prevalidator::{
    MempoolPrevalidator, MempoolPrevalidatorBasicRef, MempoolPrevalidatorMsg,
};
use crate::mempool::mempool_state::MempoolState;

pub mod mempool_prevalidator;
pub mod mempool_state;

/// In-memory synchronized struct for sharing between threads/actors
pub type CurrentMempoolStateStorageRef = Arc<RwLock<MempoolState>>;

/// Inits empty mempool state storage
pub fn init_mempool_state_storage() -> CurrentMempoolStateStorageRef {
    Arc::new(RwLock::new(MempoolState::default()))
}

#[derive(Error, Debug)]
pub enum MempoolPrevalidatorInitError {
    #[error("Mempool is disabled by configuration")]
    MempoolDisabled,
    #[error("Failed to create mempool prevalidator, reason: {reason}")]
    CreateError {
        #[from]
        reason: riker::actors::CreateError,
    },
}

pub struct MempoolPrevalidatorFactory {
    actor_system: Arc<ActorSystem>,
    log: Logger,
    persistent_storage: PersistentStorage,
    current_mempool_state: CurrentMempoolStateStorageRef,
    tezos_readonly_mempool_api: Arc<TezosApiConnectionPool>,
    /// Indicates if mempool is disabled to propagate to p2p
    pub p2p_disable_mempool: bool,
    mempool_thread_watchers: Arc<Mutex<HashMap<ActorUri, ThreadWatcher>>>,
}

impl MempoolPrevalidatorFactory {
    pub fn new(
        actor_system: Arc<ActorSystem>,
        log: Logger,
        persistent_storage: PersistentStorage,
        current_mempool_state: CurrentMempoolStateStorageRef,
        tezos_readonly_mempool_api: Arc<TezosApiConnectionPool>,
        p2p_disable_mempool: bool,
    ) -> Self {
        Self {
            actor_system,
            log,
            persistent_storage,
            current_mempool_state,
            tezos_readonly_mempool_api,
            p2p_disable_mempool,
            mempool_thread_watchers: Arc::new(Mutex::new(Default::default())),
        }
    }

    pub fn mempool_thread_watchers(&self) -> &Arc<Mutex<HashMap<ActorUri, ThreadWatcher>>> {
        &self.mempool_thread_watchers
    }

    pub fn get_or_start_mempool(
        &self,
        chain_id: ChainId,
        chain_manager: &ChainManagerRef,
    ) -> Result<MempoolPrevalidatorBasicRef, MempoolPrevalidatorInitError> {
        if self.p2p_disable_mempool {
            warn!(
                self.log,
                "Mempool is disabled by configuration, so do not start one"
            );
            Err(MempoolPrevalidatorInitError::MempoolDisabled)
        } else {
            // check if exists any for the chain_id
            if let Some(existing_mempool_prevalidator) = self.find_mempool_prevalidator(&chain_id) {
                info!(self.log, "Found already started mempool prevalidator"; "chain_id" => chain_id.to_base58_check());
                return Ok(existing_mempool_prevalidator);
            }

            // at first lock registry
            let mut mempool_thread_watchers = match self.mempool_thread_watchers.lock() {
                Ok(mempool_thread_watchers) => mempool_thread_watchers,
                Err(e) => {
                    warn!(self.log, "Failed to lock mempool thread registry"; "reason" => format!("{}", e));
                    return Err(MempoolPrevalidatorInitError::CreateError {
                        reason: CreateError::System,
                    });
                }
            };

            // if not found, we need to start new one
            info!(self.log, "Starting mempool prevalidator"; "chain_id" => chain_id.to_base58_check());
            let (mempool_prevalidator, mempool_prevalidator_thread_watcher) =
                MempoolPrevalidator::actor(
                    self.actor_system.as_ref(),
                    chain_manager.clone(),
                    self.persistent_storage.clone(),
                    self.current_mempool_state.clone(),
                    chain_id,
                    self.tezos_readonly_mempool_api.clone(),
                    self.log.clone(),
                )
                .map_err(MempoolPrevalidatorInitError::from)?;

            // add thread watcher to registry
            let _ = mempool_thread_watchers.insert(
                mempool_prevalidator.uri().clone(),
                mempool_prevalidator_thread_watcher,
            );

            Ok(MempoolPrevalidatorBasicRef::from(mempool_prevalidator))
        }
    }

    fn find_mempool_prevalidator(&self, chain_id: &ChainId) -> Option<BasicActorRef> {
        let expected_prevalidator_name = MempoolPrevalidator::name(chain_id);
        self.actor_system
            .user_root()
            .children()
            .find(|actor_ref| expected_prevalidator_name.eq(actor_ref.name()))
    }

    pub fn find_mempool_prevalidators(&self) -> Result<Vec<Prevalidator>, UnexpectedError> {
        // find potential actors
        let prevalidator_actors = self
            .actor_system
            .user_root()
            .children()
            .filter(|actor_ref| {
                MempoolPrevalidator::is_mempool_prevalidator_actor_name(actor_ref.name())
            })
            .collect::<Vec<_>>();

        if !prevalidator_actors.is_empty() {
            // resolve active prevalidators
            let mut result = Vec::with_capacity(prevalidator_actors.len());
            for prevalidator_actor in prevalidator_actors {
                // get mempool state
                let mempool_state =
                    self.current_mempool_state
                        .read()
                        .map_err(|e| UnexpectedError {
                            reason: format!("{}", e),
                        })?;
                if let Some(mempool_prevalidator) = mempool_state.prevalidator() {
                    let prevalidator_actor_chain_id =
                        MempoolPrevalidator::resolve_chain_id_from_mempool_prevalidator_actor_name(
                            prevalidator_actor.name(),
                        );
                    let accept_mempool_prevalidator =
                        if let Some(chain_id) = prevalidator_actor_chain_id {
                            mempool_prevalidator.chain_id.to_base58_check() == *chain_id
                        } else {
                            false
                        };

                    if accept_mempool_prevalidator {
                        result.push(Prevalidator {
                            chain_id: mempool_prevalidator.chain_id.to_base58_check(),
                            status: WorkerStatus {
                                phase: WorkerStatusPhase::Running,
                                since: {
                                    match mempool_state.prevalidator_started() {
                                        Some(since) => {
                                            since.to_rfc3339_opts(SecondsFormat::Millis, true)
                                        }
                                        // TODO: here should be exact date of _mempool_prevalidator_actor, not system at all
                                        None => self
                                            .actor_system
                                            .start_date()
                                            .to_rfc3339_opts(SecondsFormat::Millis, true),
                                    }
                                },
                            },
                        })
                    }
                }
            }
            Ok(result)
        } else {
            Ok(vec![])
        }
    }

    pub fn find_mempool_prevalidator_caller(
        &self,
        chain_id: &ChainId,
    ) -> Option<PrevalidatorCaller> {
        self.find_mempool_prevalidator(chain_id)
            .map(PrevalidatorCaller)
    }
}

/// Very simple wrapper on mempool prevalidator actor
pub struct PrevalidatorCaller(BasicActorRef);

impl PrevalidatorCaller {
    pub fn try_tell(&self, msg: MempoolPrevalidatorMsg) -> Result<(), UnsupportedMessageError> {
        self.0
            .try_tell(msg, None)
            .map_err(|_| UnsupportedMessageError)
    }
}

impl MempoolPrevalidatorCaller for PrevalidatorCaller {
    fn try_tell(&self, msg: MempoolRequestMessage) -> Result<(), UnsupportedMessageError> {
        match msg {
            MempoolRequestMessage::MempoolOperationReceived(msg) => {
                self.try_tell(MempoolPrevalidatorMsg::MempoolOperationReceived(msg))
            }
            MempoolRequestMessage::ResetMempool(msg) => {
                self.try_tell(MempoolPrevalidatorMsg::ResetMempool(msg))
            }
        }
    }
}
