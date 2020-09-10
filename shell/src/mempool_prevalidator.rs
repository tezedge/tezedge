// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This actor responsibility is to take care of Mempool/MempoolState,
//! which means, to validate operations which are not yet injected in any block.
//!
//! This actor listens on shell events (see [process_shell_channel_message]) and schedules it to internal queue/channel for validation processing.
//!
//! Actor validates received operations and result of validate as a new MempoolState is send back to shell channel, where:
//!     - is used by rpc_actor to show current mempool state - pending_operations
//!     - is used by chain_manager to send new current head with current mempool to inform other peers throught P2P

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver as QueueReceiver, Sender as QueueSender};
use std::thread;
use std::thread::JoinHandle;

use failure::{Error, Fail};
use riker::actors::*;
use slog::{debug, info, Logger, trace, warn};

use crypto::hash::{BlockHash, ChainId, HashType, OperationHash};
use storage::{BlockStorage, BlockStorageReader, MempoolStorage, StorageError, StorageInitInfo};
use storage::chain_meta_storage::{ChainMetaStorage, ChainMetaStorageReader};
use storage::mempool_storage::MempoolOperationType;
use storage::persistent::PersistentStorage;
use tezos_api::ffi::{Applied, BeginConstructionRequest, Errored, PrevalidatorWrapper, ValidateOperationRequest, ValidateOperationResult};
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tezos_messages::p2p::encoding::prelude::Operation;
use tezos_wrapper::service::{ProtocolController, ProtocolServiceError};
use tezos_wrapper::TezosApiConnectionPool;

use crate::shell_channel::{CurrentMempoolState, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use crate::subscription::subscribe_to_shell_events;

type SharedJoinHandle = Arc<Mutex<Option<JoinHandle<Result<(), Error>>>>>;

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(ShellChannelMsg)]
pub struct MempoolPrevalidator {
    /// All events from shell will be published to this channel
    shell_channel: ShellChannelRef,

    validator_event_sender: Arc<Mutex<QueueSender<Event>>>,
    validator_run: Arc<AtomicBool>,
    validator_thread: SharedJoinHandle,
}

enum Event {
    NewHead(BlockHash, Arc<BlockHeader>),
    ValidateOperation(OperationHash, MempoolOperationType),
    ShuttingDown,
}

/// Reference to [chain feeder](ChainFeeder) actor
pub type MempoolPrevalidatorRef = ActorRef<MempoolPrevalidatorMsg>;

impl MempoolPrevalidator {
    pub fn actor(
        sys: &impl ActorRefFactory,
        shell_channel: ShellChannelRef,
        persistent_storage: &PersistentStorage,
        init_storage_data: &StorageInitInfo,
        tezos_readonly_api: Arc<TezosApiConnectionPool>,
        log: Logger) -> Result<MempoolPrevalidatorRef, CreateError> {

        // spawn thread which processes event
        let (validator_event_sender, mut validator_event_receiver) = channel();
        let validator_run = Arc::new(AtomicBool::new(true));
        let validator_thread = {
            let persistent_storage = persistent_storage.clone();
            let shell_channel = shell_channel.clone();
            let validator_run = validator_run.clone();
            let chain_id = init_storage_data.chain_id.clone();

            thread::spawn(move || {
                let mut block_storage = BlockStorage::new(&persistent_storage);
                let mut chain_meta_storage = ChainMetaStorage::new(&persistent_storage);
                let mut mempool_storage = MempoolStorage::new(&persistent_storage);

                while validator_run.load(Ordering::Acquire) {
                    match tezos_readonly_api.pool.get() {
                        Ok(protocol_controller) =>
                            match process_prevalidation(
                                &mut block_storage,
                                &mut chain_meta_storage,
                                &mut mempool_storage,
                                &chain_id,
                                &validator_run,
                                &shell_channel,
                                &protocol_controller.api,
                                &mut validator_event_receiver,
                                &log,
                            ) {
                                Ok(()) => info!(log, "Mempool - prevalidation process finished"),
                                Err(err) => {
                                    if validator_run.load(Ordering::Acquire) {
                                        warn!(log, "Mempool - error while process prevalidation"; "reason" => format!("{:?}", err));
                                    }
                                }
                            }
                        Err(err) => warn!(log, "Mempool - no protocol runner connection available (try next turn)!"; "pool_name" => tezos_readonly_api.pool_name.clone(), "reason" => format!("{:?}", err)),
                    }
                }

                info!(log, "Mempool prevalidator thread finished");
                Ok(())
            })
        };

        // create actor
        let myself = sys.actor_of_props::<MempoolPrevalidator>(
            MempoolPrevalidator::name(),
            Props::new_args((shell_channel, validator_run, Arc::new(Mutex::new(Some(validator_thread))), Arc::new(Mutex::new(validator_event_sender)))),
        )?;

        Ok(myself)
    }

    /// The `MempoolPrevalidator` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "mempool-prevalidator"
    }

    fn process_shell_channel_message(&mut self, _: &Context<MempoolPrevalidatorMsg>, msg: ShellChannelMsg) -> Result<(), Error> {
        match msg {
            ShellChannelMsg::NewCurrentHead(head, block) => {
                // add NewHead to queue
                self.validator_event_sender.lock().unwrap().send(
                    Event::NewHead(head.hash, block.header().header.clone())
                )?;
            }
            ShellChannelMsg::MempoolOperationReceived(operation) => {
                // add operation to queue for validation
                self.validator_event_sender.lock().unwrap().send(
                    Event::ValidateOperation(operation.operation_hash.clone(), operation.operation_type)
                )?;
            }
            ShellChannelMsg::ShuttingDown(_) => {
                self.validator_event_sender.lock().unwrap().send(
                    Event::ShuttingDown
                )?;
            }
            _ => ()
        }

        Ok(())
    }
}

impl ActorFactoryArgs<(ShellChannelRef, Arc<AtomicBool>, SharedJoinHandle, Arc<Mutex<QueueSender<Event>>>)> for MempoolPrevalidator {
    fn create_args((shell_channel, validator_run, validator_thread, validator_event_sender): (ShellChannelRef, Arc<AtomicBool>, SharedJoinHandle, Arc<Mutex<QueueSender<Event>>>)) -> Self {
        MempoolPrevalidator {
            shell_channel,
            validator_run,
            validator_thread,
            validator_event_sender,
        }
    }
}

impl Actor for MempoolPrevalidator {
    type Msg = MempoolPrevalidatorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_events(&self.shell_channel, ctx.myself());
    }

    fn post_stop(&mut self) {
        self.validator_run.store(false, Ordering::Release);

        let join_handle = self.validator_thread.lock().unwrap()
            .take().expect("Thread join handle is missing");
        join_handle.thread().unpark();
        let _ = join_handle.join().expect("Failed to join block applier thread");
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ShellChannelMsg> for MempoolPrevalidator {
    type Msg = MempoolPrevalidatorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match self.process_shell_channel_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => warn!(ctx.system.log(), "Mempool - failed to process shell channel message"; "reason" => format!("{:?}", e)),
        }
    }
}

/// Mempool state is defined with mempool and validation_result attriibutes, which are in sync:
/// - `validation_result`
///     - contains results of all validated operations
///     - also contains `known_valid` operations, which where validated as `applied`
/// - `pending`
///     - operations, which where not validated yet or endorsements (`branch_refused`, `branched_delay`, `refused`?)
///     - are being processed sequentially, after validation, they are moved to `validation_result`
/// - `operations`
///     - kind of cache, contains operation data
#[derive(Clone, Debug)]
pub struct MempoolState {
    /// Original tezos prevalidator has prevalidator.fitness which is used for set_head comparision
    /// So, we keep it in-memory here
    prevalidator: Option<PrevalidatorWrapper>,
    predecessor: Option<BlockHash>,

    /// Actual cumulated operation results
    validation_result: ValidateOperationResult,

    /// In-memory store of actual operations
    operations: HashMap<OperationHash, Operation>,
    // TODO: pendings limit
    pending: HashSet<OperationHash>,
}

impl MempoolState {
    fn new(prevalidator: Option<PrevalidatorWrapper>, predecessor: Option<BlockHash>, pending_operations: HashMap<OperationHash, Operation>) -> MempoolState {
        MempoolState {
            prevalidator,
            predecessor,
            pending: pending_operations.keys().cloned().collect(),
            validation_result: ValidateOperationResult::default(),
            operations: pending_operations,
        }
    }

    fn add_result(&mut self, new_result: &ValidateOperationResult) -> bool {
        self.validation_result.merge(&new_result)
    }

    fn add_to_pending(&mut self, operation_hash: &OperationHash, operation: &Operation) {
        self.operations.insert(operation_hash.clone(), operation.clone());
        let _ = self.pending.insert(operation_hash.clone());
    }

    fn remove_from_pending(&mut self, operation_hash: &OperationHash) -> bool {
        self.pending.remove(operation_hash)
    }

    /// Indicates, that pending operations can be handled
    fn can_handle_pending(&self) -> bool {
        !self.pending.is_empty() && self.prevalidator.is_some()
    }

    /// Indicates, that the operation was allready validated and is in the mempool
    fn is_already_validated(&self, operation_hash: &OperationHash) -> bool {
        let mut contains = self.validation_result.applied.clone().into_iter().filter(|k| &k.hash == operation_hash).collect::<Vec<Applied>>().is_empty();
        contains &= self.validation_result.refused.clone().into_iter().filter(|k| &k.hash == operation_hash).collect::<Vec<Errored>>().is_empty();
        contains &= self.validation_result.branch_refused.clone().into_iter().filter(|k| &k.hash == operation_hash).collect::<Vec<Errored>>().is_empty();
        contains &= self.validation_result.branch_delayed.clone().into_iter().filter(|k| &k.hash == operation_hash).collect::<Vec<Errored>>().is_empty();
        !contains
    }

    /// Splits exists operations map to operations map with just pending operations and others
    fn split_operations_to_pending_and_others(&self) -> (HashMap<OperationHash, Operation>, HashSet<OperationHash>) {
        let mut pending = HashMap::new();
        let mut others = HashSet::new();

        // split
        for (key, value) in self.operations.iter() {
            if self.pending.contains(key) {
                pending.insert(key.clone(), value.clone());
            } else {
                others.insert(key.clone());
            }
        }

        (pending, others)
    }
}

/// Possible errors for prevalidation
#[derive(Debug, Fail)]
pub enum PrevalidationError {
    #[fail(display = "Storage read/write error! Reason: {:?}", error)]
    StorageError {
        error: StorageError
    },
    #[fail(display = "Protocol service error error! Reason: {:?}", error)]
    ProtocolServiceError {
        error: ProtocolServiceError
    },
}

impl From<ProtocolServiceError> for PrevalidationError {
    fn from(error: ProtocolServiceError) -> Self {
        PrevalidationError::ProtocolServiceError { error }
    }
}

impl From<StorageError> for PrevalidationError {
    fn from(error: StorageError) -> Self {
        PrevalidationError::StorageError { error }
    }
}

fn process_prevalidation(
    block_storage: &BlockStorage,
    chain_meta_storage: &ChainMetaStorage,
    mempool_storage: &MempoolStorage,
    chain_id: &ChainId,
    validator_run: &AtomicBool,
    shell_channel: &ShellChannelRef,
    protocol_controller: &ProtocolController,
    validator_event_receiver: &mut QueueReceiver<Event>,
    log: &Logger,
) -> Result<(), PrevalidationError> {
    info!(log, "Mempool prevalidator started processing");

    // hydrate state
    let mut state = hydrate_state(
        &shell_channel,
        block_storage,
        chain_meta_storage,
        mempool_storage,
        &protocol_controller,
        &chain_id,
        &log,
    )?;

    // start receiving event
    while validator_run.load(Ordering::Acquire) {
        // 1. at first let's handle event
        if let Ok(event) = validator_event_receiver.recv() {
            match event {
                Event::NewHead(header_hash, header) => {
                    debug!(log, "Mempool - new head received, so begin construction a new context";
                                "received_block_hash" => HashType::BlockHash.bytes_to_string(&header_hash));

                    // try to begin construction new context
                    let (prevalidator, head) = begin_construction(&protocol_controller, &chain_id, &header_hash, header, &log)?;

                    // recreate state, reuse just pendings
                    let (pending_operations, mut operations_to_delete) = state.split_operations_to_pending_and_others();
                    state = MempoolState::new(prevalidator, head, pending_operations);

                    // notify other actors
                    notify_mempool_changed(&shell_channel, &state);

                    // clear unneeded operations from mempool storage
                    operations_to_delete
                        .drain()
                        .for_each(|oph| {
                            if let Err(err) = mempool_storage.delete(&oph) {
                                warn!(log, "Mempool - delete operation failed"; "hash" => HashType::OperationHash.bytes_to_string(&oph), "error" => format!("{:?}", err))
                            }
                        });
                }
                Event::ValidateOperation(oph, mempool_operation_type) => {
                    // TODO: handling when operation not exists - can happen?
                    let operation = mempool_storage.get(mempool_operation_type, oph.clone())?;
                    if let Some(operation) = operation {

                        // TODO: handle and validate pre_filter with operation?

                        if state.is_already_validated(&oph) {
                            debug!(log, "Mempool - received validate operation event - operation already validated"; "hash" => HashType::OperationHash.bytes_to_string(&oph));
                        } else {
                            // just add operations to pendings
                            state.add_to_pending(&oph, operation.operation());
                        }
                    } else {
                        debug!(log, "Mempool - received validate operation event - operations was previously validated and removed from mempool storage"; "hash" => HashType::OperationHash.bytes_to_string(&oph));
                    }
                }
                Event::ShuttingDown => {
                    validator_run.store(false, Ordering::Release);
                }
            }
        }

        // 2. lets handle pending operations (if any)
        handle_pending_operations(&shell_channel, &protocol_controller, &mut state, &log);
    }

    Ok(())
}

fn hydrate_state(
    shell_channel: &ShellChannelRef,
    block_storage: &BlockStorage,
    chain_meta_storage: &ChainMetaStorage,
    mempool_storage: &MempoolStorage,
    protocol_controller: &ProtocolController,
    chain_id: &ChainId,
    log: &Logger) -> Result<MempoolState, PrevalidationError> {

    // load current head
    let current_head = match chain_meta_storage.get_current_head(&chain_id)? {
        Some(head) => block_storage.get(&head.hash)?.map(|header| (head, header.header)),
        None => None,
    };

    // begin construction for a current head
    let (prevalidator, head) = match current_head {
        Some((head, header)) => begin_construction(protocol_controller, &chain_id, &head.hash, header, &log)?,
        None => (None, None)
    };

    // read from Mempool_storage (just pending) -> add to queue for validation -> pending
    let pending = mempool_storage.iter()?
        .into_iter()
        .map(|(key, value)| (key, value.operation().clone()))
        .collect();

    // internal mempool state
    let mut state = MempoolState::new(prevalidator, head, pending);

    // TODO: do we need this?
    // and process it immediatly on startup, before any event received to clean old stored unprocessed operations
    if state.can_handle_pending() {
        handle_pending_operations(&shell_channel, &protocol_controller, &mut state, &log);
    }

    Ok(state)
}

fn begin_construction(protocol_controller: &ProtocolController,
                      chain_id: &ChainId,
                      block_hash: &BlockHash,
                      block_header: Arc<BlockHeader>,
                      log: &Logger) -> Result<(Option<PrevalidatorWrapper>, Option<BlockHash>), PrevalidationError> {

    // try to begin construction
    let result = match protocol_controller.begin_construction(
        BeginConstructionRequest {
            chain_id: chain_id.clone(),
            predecessor: (&*block_header).clone(),
            protocol_data: None,
        }
    ) {
        Ok(prevalidator) => (
            Some(prevalidator),
            Some(block_hash.clone()),
        ),
        Err(err) => {
            warn!(log, "Mempool - failed to begin construction"; "block_hash" => HashType::BlockHash.bytes_to_string(&block_hash), "error" => format!("{:?}", err));
            (None, None)
        }
    };
    Ok(result)
}

fn handle_pending_operations(shell_channel: &ShellChannelRef, protocol_controller: &ProtocolController, state: &mut MempoolState, log: &Logger) {
    debug!(log, "Mempool - handle_pending_operations "; "pendings" => state.pending.len(), "can" => state.can_handle_pending());

    if !state.can_handle_pending() {
        trace!(log, "Mempool - handle_pending_operations - nothing to handle");
        return;
    }

    let prevalidator = if let Some(prevalidator) = &state.prevalidator {
        prevalidator.clone()
    } else {
        // no prevalidator, means nothing to do
        return;
    };

    // lets iterate pendings and validate them
    let mut state_changed = false;
    let mut pending_ops = state.pending.clone();
    pending_ops
        .drain()
        .for_each(|pending_op| {
            match state.operations.get(&pending_op) {
                Some(operation) => {
                    trace!(log, "Mempool - lets validate "; "hash" => HashType::OperationHash.bytes_to_string(&pending_op));

                    // lets validate throught protocol
                    match protocol_controller.validate_operation(
                        ValidateOperationRequest {
                            prevalidator: prevalidator.clone(),
                            operation: operation.clone(),
                        }
                    ) {
                        Ok(response) => {
                            let result = response.result;
                            debug!(log, "Mempool - validate operation response finished with success "; "hash" => HashType::OperationHash.bytes_to_string(&pending_op), "result" => format!("{:?}", result));

                            // merge new result with existing one
                            state_changed |= state.add_result(&result);

                            // TODO: handle Duplicate/ Outdated - if result is empty
                            // TODO: handle result like ocaml - branch_delayed (is_endorsement) add back to pending and so on - check handle_unprocessed
                        }
                        Err(err) => {
                            warn!(log, "Mempool - failed to validate operation message"; "hash" => HashType::OperationHash.bytes_to_string(&pending_op), "error" => format!("{:?}", err));
                            // TODO: create custom error and add to refused or just revalidate (retry algorithm?)
                            // TODO: handle error?
                        }
                    }

                    // remove from pendings
                    state_changed |= state.remove_from_pending(&pending_op);
                }
                None => warn!(log, "Mempool - missing operation in mempool state (should not happen)"; "hash" => HashType::OperationHash.bytes_to_string(&pending_op))
            }
        });

    // lets notify actors about changed mempool
    if state_changed {
        notify_mempool_changed(&shell_channel, &state);
    }
}

/// Notify other actors that mempool state changed
fn notify_mempool_changed(shell_channel: &ShellChannelRef, mempool_state: &MempoolState) {
    let (protocol, fitness) = if let Some(prevalidator) = &mempool_state.prevalidator {
        (Some(prevalidator.protocol.clone()), prevalidator.context_fitness.clone())
    } else {
        (None, None)
    };

    shell_channel.tell(
        Publish {
            msg: CurrentMempoolState {
                head: mempool_state.predecessor.clone(),
                result: mempool_state.validation_result.clone(),
                operations: mempool_state.operations.clone(),
                protocol,
                fitness,
                pending: mempool_state.pending.clone(),
            }.into(),
            topic: ShellChannelTopic::ShellEvents.into(),
        },
        None,
    );
}

