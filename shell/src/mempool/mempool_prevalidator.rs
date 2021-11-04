// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This actor responsibility is to take care of Mempool/MempoolState,
//! which means, to validate operations which are not yet injected in any block.
//!
//! Actor validates received operations and result of validate as a new MempoolState is send back to shell channel, where:
//!     - is used by rpc_actor to show current mempool state - pending_operations
//!     - is used by chain_manager to send new current head with current mempool to inform other peers throught P2P

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver as QueueReceiver, Sender as QueueSender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use anyhow::{format_err, Error};
use slog::{debug, info, trace, warn, Logger};
use tezedge_actor_system::actors::*;
use tezos_protocol_ipc_client::{
    handle_protocol_service_error, ProtocolRunnerApi, ProtocolRunnerConnection,
    ProtocolServiceError,
};
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId};
use shell_integration::{
    dispatch_oneshot_result, MempoolError, MempoolOperationReceived, ResetMempool,
};
use shell_integration::{StreamCounter, ThreadRunningStatus, ThreadWatcher};
use storage::chain_meta_storage::{ChainMetaStorage, ChainMetaStorageReader};
use storage::{BlockHeaderWithHash, PersistentStorage};
use storage::{BlockStorage, BlockStorageReader, StorageError};
use tezos_api::ffi::{BeginConstructionRequest, PrevalidatorWrapper, ValidateOperationRequest};
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use crate::chain_manager::{AdvertiseToP2pNewMempool, ChainManagerRef};
use crate::mempool::CurrentMempoolStateStorageRef;

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(ResetMempool, MempoolOperationReceived)]
pub struct MempoolPrevalidator {
    validator_event_sender: Arc<Mutex<QueueSender<Event>>>,
    validator_run: ThreadRunningStatus,
}

enum Event {
    NewHead(Arc<BlockHeaderWithHash>),
    ValidateOperation(MempoolOperationReceived),
    ShuttingDown,
}

pub type MempoolPrevalidatorRef = ActorRef<MempoolPrevalidatorMsg>;
pub type MempoolPrevalidatorBasicRef = BasicActorRef;

impl MempoolPrevalidator {
    pub fn actor(
        sys: &impl ActorRefFactory,
        chain_manager: ChainManagerRef,
        persistent_storage: PersistentStorage,
        current_mempool_state_storage: CurrentMempoolStateStorageRef,
        chain_id: ChainId,
        tezos_protocol_api: Arc<ProtocolRunnerApi>,
        log: Logger,
    ) -> Result<(MempoolPrevalidatorRef, ThreadWatcher), CreateError> {
        let thread_name = format!("mmpl-{}", chain_id.to_base58_check());

        // spawn thread which processes event
        let (validator_event_sender, mut validator_event_receiver) = channel();
        let mut mempool_thread_watcher = {
            let validator_event_sender = validator_event_sender.clone();
            ThreadWatcher::start(
                thread_name.clone(),
                Box::new(move || {
                    validator_event_sender
                        .send(Event::ShuttingDown)
                        .map_err(|e| e.into())
                }),
            )
        };

        let mempool_thread = {
            let validator_run = mempool_thread_watcher.thread_running_status().clone();
            let chain_id = chain_id.clone();
            let tokio_runtime = tezos_protocol_api.tokio_runtime.clone();

            thread::Builder::new().name(thread_name).spawn(move || {
                let block_storage = BlockStorage::new(&persistent_storage);
                let chain_meta_storage = ChainMetaStorage::new(&persistent_storage);

                while validator_run.load(Ordering::Acquire) {
                    match tezos_protocol_api.readable_connection_sync() {
                        Ok(mut protocol_controller) => match process_prevalidation(
                            &block_storage,
                            &chain_meta_storage,
                            &current_mempool_state_storage,
                            &chain_id,
                            &validator_run,
                            &chain_manager,
                            &mut protocol_controller,
                            &tokio_runtime,
                            &mut validator_event_receiver,
                            &log,
                        ) {
                            Ok(()) => {
                                info!(log, "Mempool - prevalidation process finished")
                            }
                            Err(err) => {
                                if validator_run.load(Ordering::Acquire) {
                                    warn!(log, "Mempool - error while process prevalidation"; "reason" => format!("{:?}", err));
                                }
                            }
                        },
                        Err(err) => {
                            // TODO: this cannot happen anymore, there are no pools and protocol runners always accept connections
                            warn!(log, "Mempool - no protocol runner connection available (try next turn)!"; "reason" => format!("{:?}", err))
                        }
                    }
                }

                info!(log, "Mempool prevalidator thread finished");
            }).map_err(|_|{CreateError::Panicked})?
        };
        mempool_thread_watcher.set_thread(mempool_thread);

        // create actor
        let myself = sys.actor_of_props::<MempoolPrevalidator>(
            &MempoolPrevalidator::name(&chain_id),
            Props::new_args((
                mempool_thread_watcher.thread_running_status().clone(),
                Arc::new(Mutex::new(validator_event_sender)),
            )),
        )?;

        Ok((myself, mempool_thread_watcher))
    }

    fn process_reset_mempool_message(
        &mut self,
        _: &Context<MempoolPrevalidatorMsg>,
        msg: ResetMempool,
    ) -> Result<(), Error> {
        let ResetMempool { block } = msg;
        // add NewHead to queue
        self.validator_event_sender
            .lock()
            .map_err(|e| format_err!("Failed to obtain the lock: {:?}", e))?
            .send(Event::NewHead(block))?;
        Ok(())
    }

    fn process_mempool_operation_received_message(
        &mut self,
        _: &Context<MempoolPrevalidatorMsg>,
        msg: MempoolOperationReceived,
    ) -> Result<(), Error> {
        // add operation to queue for validation
        self.validator_event_sender
            .lock()
            .map_err(|e| format_err!("Failed to obtain the lock: {:?}", e))?
            .send(Event::ValidateOperation(msg))?;
        Ok(())
    }

    const PREFIX_NAME: &'static str = "mempool-prevalidator-";

    /// The `MempoolPrevalidator` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    ///
    /// e.g.: mempool-prevalidator-NetXdQprcVkpaWU
    pub fn name(chain_id: &ChainId) -> String {
        format!("{}{}", Self::PREFIX_NAME, chain_id.to_base58_check())
    }

    pub fn is_mempool_prevalidator_actor_name(actor_name: &str) -> bool {
        actor_name.starts_with(Self::PREFIX_NAME)
    }

    /// Returns chain_id in base58_chech format
    pub fn resolve_chain_id_from_mempool_prevalidator_actor_name(actor_name: &str) -> Option<&str> {
        if let Some(rest) = actor_name.strip_prefix(Self::PREFIX_NAME) {
            Some(rest)
        } else {
            None
        }
    }
}

impl ActorFactoryArgs<(ThreadRunningStatus, Arc<Mutex<QueueSender<Event>>>)>
    for MempoolPrevalidator
{
    fn create_args(
        (validator_run, validator_event_sender): (
            ThreadRunningStatus,
            Arc<Mutex<QueueSender<Event>>>,
        ),
    ) -> Self {
        MempoolPrevalidator {
            validator_run,
            validator_event_sender,
        }
    }
}

impl Actor for MempoolPrevalidator {
    type Msg = MempoolPrevalidatorMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<MempoolOperationReceived> for MempoolPrevalidator {
    type Msg = MempoolPrevalidatorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: MempoolOperationReceived, _: Sender) {
        // do not process any message, when thread is down
        if !self.validator_run.load(Ordering::Acquire) {
            return;
        }
        match self.process_mempool_operation_received_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Mempool - failed to process `MempoolOperationReceived` message"; "reason" => format!("{:?}", e))
            }
        }
    }
}

impl Receive<ResetMempool> for MempoolPrevalidator {
    type Msg = MempoolPrevalidatorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ResetMempool, _: Sender) {
        // do not process any message, when thread is down
        if !self.validator_run.load(Ordering::Acquire) {
            return;
        }
        match self.process_reset_mempool_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Mempool - failed to process `ResetMempool` message"; "reason" => format!("{:?}", e))
            }
        }
    }
}

/// Possible errors for prevalidation
#[derive(Debug, Error)]
pub enum PrevalidationError {
    #[error("Storage read/write error, reason: {error:?}")]
    StorageError { error: StorageError },
    #[error("Protocol service error, reason: {error:?}")]
    ProtocolServiceError {
        #[from]
        error: ProtocolServiceError,
    },
    #[error("Current mempool storage lock error, reason: {reason:?}")]
    CurrentMempoolStorageLockError { reason: String },
}

impl From<StorageError> for PrevalidationError {
    fn from(error: StorageError) -> Self {
        PrevalidationError::StorageError { error }
    }
}

impl<T> From<PoisonError<T>> for PrevalidationError {
    fn from(pe: PoisonError<T>) -> Self {
        PrevalidationError::CurrentMempoolStorageLockError {
            reason: format!("{}", pe),
        }
    }
}

fn process_prevalidation(
    block_storage: &BlockStorage,
    chain_meta_storage: &ChainMetaStorage,
    current_mempool_state_storage: &CurrentMempoolStateStorageRef,
    chain_id: &ChainId,
    validator_run: &AtomicBool,
    chain_manager: &ChainManagerRef,
    api: &mut ProtocolRunnerConnection,
    tokio_runtime: &tokio::runtime::Handle,
    validator_event_receiver: &mut QueueReceiver<Event>,
    log: &Logger,
) -> Result<(), PrevalidationError> {
    info!(log, "Mempool prevalidator started processing");

    // hydrate state
    hydrate_state(
        chain_manager,
        block_storage,
        chain_meta_storage,
        current_mempool_state_storage,
        api,
        tokio_runtime,
        chain_id,
        log,
    )?;

    // start receiving event
    while validator_run.load(Ordering::Acquire) {
        // 1. at first let's handle event
        if let Ok(event) = validator_event_receiver.recv() {
            match event {
                Event::NewHead(header) => {
                    // we dont want to reset mempool if header is not changed
                    let process_new_head = match current_mempool_state_storage.read()?.head() {
                        Some(mempool_head) => mempool_head.ne(&header.hash),
                        None => true,
                    };

                    if process_new_head {
                        debug!(log, "Mempool - new head received, so begin construction a new context"; "received_block_hash" => header.hash.to_base58_check());

                        // try to begin construction new context
                        let (prevalidator, head) = begin_construction(
                            api,
                            tokio_runtime,
                            chain_id,
                            header.hash.clone(),
                            header.header.clone(),
                            log,
                        )?;

                        // reinitialize state for new prevalidator and head
                        let _ = current_mempool_state_storage
                            .write()?
                            .reinit(prevalidator, head);
                    } else {
                        debug!(log, "Mempool - new head received, but was ignored"; "received_block_hash" => header.hash.to_base58_check());
                    }
                }
                Event::ValidateOperation(MempoolOperationReceived {
                    operation_hash: oph,
                    operation,
                    result_callback,
                }) => {
                    // try to add to pendings
                    let was_added_to_pending = current_mempool_state_storage
                        .write()?
                        .add_to_pending(&oph, operation);
                    if !was_added_to_pending {
                        debug!(log, "Mempool - received validate operation event - operation already validated"; "operation_hash" => oph.to_base58_check());
                        if let Err(e) = dispatch_oneshot_result(result_callback, || {
                            Err(MempoolError {reason: format!("Mempool - received validate operation event - operation already validated, hash: {}", oph.to_base58_check())})
                        }) {
                            warn!(log, "Failed to dispatch result"; "reason" => format!("{}", e));
                        }
                    } else if let Err(e) = dispatch_oneshot_result(result_callback, || Ok(())) {
                        warn!(log, "Failed to dispatch result"; "reason" => format!("{}", e));
                    } else {
                        current_mempool_state_storage.read()?.wake_up_all_streams();
                    }
                }
                Event::ShuttingDown => {
                    // just finish the loop
                    info!(
                        log,
                        "Mempool prevalidator thread worker received shutting down event"
                    );
                    break;
                }
            }
        }

        // 2. lets handle pending operations (if any)
        handle_pending_operations(
            chain_manager,
            api,
            tokio_runtime,
            current_mempool_state_storage,
            log,
        )?;
    }

    Ok(())
}

fn hydrate_state(
    chain_manager: &ChainManagerRef,
    block_storage: &BlockStorage,
    chain_meta_storage: &ChainMetaStorage,
    current_mempool_state_storage: &CurrentMempoolStateStorageRef,
    api: &mut ProtocolRunnerConnection,
    tokio_runtime: &tokio::runtime::Handle,
    chain_id: &ChainId,
    log: &Logger,
) -> Result<(), PrevalidationError> {
    // load current head
    let current_head = match chain_meta_storage.get_current_head(chain_id)? {
        Some(head) => block_storage
            .get(head.block_hash())?
            .map(|header| (head, header.header)),
        None => None,
    };

    // begin construction for a current head
    let (prevalidator, head) = match current_head {
        Some((head, header)) => {
            begin_construction(api, tokio_runtime, chain_id, head.into(), header, log)?
        }
        None => (None, None),
    };

    // initialize internal mempool state (write lock)
    let mut mempool_state = current_mempool_state_storage.write()?;

    // reinit + add old unprocessed pendings
    let _ = mempool_state.reinit(prevalidator, head);

    // ste started date
    if mempool_state.prevalidator_started().is_none() {
        mempool_state.set_prevalidator_started();
    }

    // release lock asap
    drop(mempool_state);

    // and process it immediatly on startup, before any event received to clean old stored unprocessed operations
    handle_pending_operations(
        chain_manager,
        api,
        tokio_runtime,
        current_mempool_state_storage,
        log,
    )?;

    Ok(())
}

fn begin_construction(
    api: &mut ProtocolRunnerConnection,
    tokio_runtime: &tokio::runtime::Handle,
    chain_id: &ChainId,
    block_hash: BlockHash,
    block_header: Arc<BlockHeader>,
    log: &Logger,
) -> Result<(Option<PrevalidatorWrapper>, Option<BlockHash>), PrevalidationError> {
    let result = tokio::task::block_in_place(|| {
        tokio_runtime.block_on(
            api.begin_construction_for_mempool(BeginConstructionRequest {
                chain_id: chain_id.clone(),
                predecessor: block_header.as_ref().clone(),
                protocol_data: None,
            }),
        )
    });
    // try to begin construction
    let result = match result {
        Ok(prevalidator) => (Some(prevalidator), Some(block_hash)),
        Err(pse) => {
            handle_protocol_service_error(
                pse,
                |e| warn!(log, "Mempool - failed to begin construction"; "block_hash" => block_hash.to_base58_check(), "error" => format!("{:?}", e)),
            )?;
            (None, None)
        }
    };
    Ok(result)
}

fn handle_pending_operations(
    chain_manager: &ChainManagerRef,
    api: &mut ProtocolRunnerConnection,
    tokio_runtime: &tokio::runtime::Handle,
    current_mempool_state_storage: &CurrentMempoolStateStorageRef,
    log: &Logger,
) -> Result<(), PrevalidationError> {
    // check if we can handle something
    let (prevalidator, head, mut pendings, operations) = {
        let mut mempool_state = current_mempool_state_storage.write()?;
        match mempool_state.drain_pending() {
            Some((prevalidator, head, pendings, operations)) => {
                // release lock asap
                drop(mempool_state);
                debug!(log, "Mempool - handle_pending_operations"; "pendings" => pendings.len(), "mempool_head" => head.to_base58_check());
                (prevalidator, head, pendings, operations)
            }
            None => {
                trace!(
                    log,
                    "Mempool - handle_pending_operations - nothing to handle or no prevalidator"
                );
                return Ok(());
            }
        }
    };

    // lets iterate pendings and validate them
    for pending_op in pendings.drain() {
        // handle validation
        match operations.get(&pending_op) {
            Some(mempool_operation) => {
                trace!(log, "Mempool - lets validate "; "operation_hash" => pending_op.to_base58_check());

                let result = tokio::task::block_in_place(|| {
                    tokio_runtime.block_on(api.validate_operation_for_mempool(
                        ValidateOperationRequest {
                            prevalidator: prevalidator.clone(),
                            operation: mempool_operation.operation().clone(),
                        },
                    ))
                });

                // lets validate throught protocol
                match result {
                    Ok(response) => {
                        debug!(log, "Mempool - validate operation response finished with success"; "operation_hash" => pending_op.to_base58_check(), "result" => format!("{:?}", response.result));

                        // merge new result with existing one
                        // we need to aquire state after every operation, because we dont want to lock shared mutex, while waiting for ffi
                        match current_mempool_state_storage.write() {
                            Ok(mut mempool_state) => {
                                mempool_state.merge_validation_result(response.result);

                                // TODO: handle Duplicate/ Outdated - if result is empty
                                // TODO: handle result like ocaml - branch_delayed (is_endorsement) add back to pending and so on - check handle_unprocessed
                            }
                            Err(e) => {
                                warn!(log, "Failed to merge operation result to mempool"; "reason" => format!("{:?}", e), "operation_hash" => pending_op.to_base58_check(), "validation_result" => format!("{:?}", response), "mempool_head" => head.to_base58_check());
                            }
                        }
                    }
                    Err(pse) => {
                        handle_protocol_service_error(
                            pse,
                            |e| warn!(log, "Mempool - failed to validate operation message"; "operation_hash" => pending_op.to_base58_check(), "error" => format!("{:?}", e), "mempool_head" => head.to_base58_check()),
                        )?

                        // TODO: create custom error and add to refused or just revalidate (retry algorithm?)
                    }
                }
            }
            None => {
                warn!(log, "Mempool - missing operation in mempool state (should not happen)"; "operation_hash" => pending_op.to_base58_check(), "mempool_head" => head.to_base58_check())
            }
        }
    }

    // collect actual mempool
    match current_mempool_state_storage.read() {
        Ok(mempool_state) => {
            if let Some((mempool, mempool_operations)) =
                mempool_state.collect_mempool_operations_to_advertise(pendings)
            {
                // release lock asap
                drop(mempool_state);

                // advertise actual mempool
                chain_manager.tell(
                    AdvertiseToP2pNewMempool {
                        chain_id: prevalidator.chain_id,
                        mempool_head: head,
                        mempool,
                        mempool_operations,
                    },
                    None,
                );
            }
        }
        Err(e) => {
            warn!(log, "Failed to prepare mempool data for advertising"; "reason" => format!("{:?}", e), "mempool_head" => head.to_base58_check());
        }
    }

    Ok(())
}
