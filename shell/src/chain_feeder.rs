// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Sends blocks to the `protocol_runner`.
//! This actor is responsible for correct applying of blocks with Tezos protocol in context
//! This actor is aslo responsible for correct initialization of genesis in storage.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver as QueueReceiver, Sender as QueueSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use failure::{format_err, Error, Fail};
use riker::actors::*;
use slog::{debug, info, trace, warn, Logger};

use crypto::hash::{BlockHash, ChainId, ContextHash};
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::context::{ContextApi, TezedgeContext};
use storage::{block_meta_storage, BlockHeaderWithHash, BlockMetaStorageReader, PersistentStorage};
use storage::{
    initialize_storage_with_genesis_block, store_applied_block_result, store_commit_genesis_result,
    BlockMetaStorage, BlockStorage, BlockStorageReader, ChainMetaStorage, OperationsMetaStorage,
    OperationsStorage, OperationsStorageReader, StorageError, StorageInitInfo,
};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::ApplyBlockRequest;
use tezos_wrapper::service::{
    handle_protocol_service_error, ProtocolController, ProtocolServiceError,
};
use tezos_wrapper::TezosApiConnectionPool;

use crate::chain_current_head_manager::{ChainCurrentHeadManagerRef, ProcessValidatedBlock};
use crate::peer_branch_bootstrapper::{
    BlockBatchApplied, BlockBatchApplyFailed, PeerBranchBootstrapperRef,
};
use crate::shell_channel::{ShellChannelMsg, ShellChannelRef};
use crate::state::BlockApplyBatch;
use crate::stats::apply_block_stats::BlockValidationTimer;
use crate::subscription::subscribe_to_shell_shutdown;
use crate::utils::{dispatch_condvar_result, AtomicTryLockGuard, CondvarResult};

type SharedJoinHandle = Arc<Mutex<Option<JoinHandle<Result<(), Error>>>>>;

/// Message commands [`ChainFeeder`] to apply completed block.
#[derive(Clone, Debug)]
pub struct ApplyBlock {
    batch: BlockApplyBatch,
    chain_id: Arc<ChainId>,
    bootstrapper: Option<PeerBranchBootstrapperRef>,
    /// Callback can be used to wait for apply block result
    result_callback: Option<CondvarResult<(), failure::Error>>,

    /// Simple lock guard, for easy synchronization
    permit: Option<Arc<AtomicTryLockGuard>>,
}

impl ApplyBlock {
    pub fn new(
        chain_id: Arc<ChainId>,
        batch: BlockApplyBatch,
        result_callback: Option<CondvarResult<(), failure::Error>>,
        bootstrapper: Option<PeerBranchBootstrapperRef>,
        permit: Option<AtomicTryLockGuard>,
    ) -> Self {
        Self {
            chain_id,
            batch,
            result_callback,
            bootstrapper,
            permit: permit.map(Arc::new),
        }
    }
}

/// Internal queue commands
pub(crate) enum Event {
    ApplyBlock(ApplyBlock),
    ShuttingDown,
}

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(ShellChannelMsg, ApplyBlock)]
pub struct ChainFeeder {
    /// Just for subscribing to shell shutdown channel
    shell_channel: ShellChannelRef,

    /// Internal queue sender
    block_applier_event_sender: Arc<Mutex<QueueSender<Event>>>,
    /// Thread where blocks are applied will run until this is set to `false`
    block_applier_run: Arc<AtomicBool>,
    /// Block applier thread
    block_applier_thread: SharedJoinHandle,
}

/// Reference to [chain feeder](ChainFeeder) actor
pub type ChainFeederRef = ActorRef<ChainFeederMsg>;

impl ChainFeeder {
    /// Create new actor instance.
    ///
    /// If the actor is successfully created then reference to the actor is returned.
    /// Commands to the tezos protocol are transmitted via IPC channel provided by [`ipc_server`](IpcCmdServer).
    ///
    /// This actor spawns a new thread in which it will periodically monitor [`persistent_storage`](PersistentStorage).
    /// Purpose of the monitoring thread is to detect whether it is possible to apply blocks received by the p2p layer.
    /// If the block can be applied, it is sent via IPC to the `protocol_runner`, where it is then applied by calling a tezos ffi.
    pub fn actor(
        sys: &impl ActorRefFactory,
        chain_current_head_manager: ChainCurrentHeadManagerRef,
        shell_channel: ShellChannelRef,
        persistent_storage: PersistentStorage,
        tezos_writeable_api: Arc<TezosApiConnectionPool>,
        init_storage_data: StorageInitInfo,
        tezos_env: TezosEnvironmentConfiguration,
        log: Logger,
    ) -> Result<ChainFeederRef, CreateError> {
        // spawn inner thread
        let (block_applier_event_sender, block_applier_run, block_applier_thread) =
            BlockApplierThreadSpawner::new(
                chain_current_head_manager,
                persistent_storage,
                Arc::new(init_storage_data),
                Arc::new(tezos_env),
                tezos_writeable_api,
                log,
            )
            .spawn_feeder_thread();

        sys.actor_of_props::<ChainFeeder>(
            ChainFeeder::name(),
            Props::new_args((
                shell_channel,
                Arc::new(Mutex::new(block_applier_event_sender)),
                block_applier_run,
                Arc::new(Mutex::new(Some(block_applier_thread))),
            )),
        )
    }

    /// The `ChainFeeder` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "chain-feeder"
    }

    fn send_to_queue(&self, event: Event) -> Result<(), Error> {
        self.block_applier_event_sender
            .lock()
            .map_err(|e| format_err!("Failed to lock queue, reason: {}", e))?
            .send(event)
            .map_err(|e| format_err!("Failed to send to queue, reason: {}", e))
    }

    fn apply_completed_block(&self, msg: ApplyBlock, log: &Logger) {
        // add request to queue
        let result_callback = msg.result_callback.clone();
        if let Err(e) = self.send_to_queue(Event::ApplyBlock(msg)) {
            warn!(log, "Failed to send `apply block request` to queue"; "reason" => format!("{}", e));
            if let Err(e) = dispatch_condvar_result(result_callback, || Err(e), true) {
                warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
            }
        }
    }
}

impl
    ActorFactoryArgs<(
        ShellChannelRef,
        Arc<Mutex<QueueSender<Event>>>,
        Arc<AtomicBool>,
        SharedJoinHandle,
    )> for ChainFeeder
{
    fn create_args(
        (shell_channel, block_applier_event_sender, block_applier_run, block_applier_thread): (
            ShellChannelRef,
            Arc<Mutex<QueueSender<Event>>>,
            Arc<AtomicBool>,
            SharedJoinHandle,
        ),
    ) -> Self {
        ChainFeeder {
            shell_channel,
            block_applier_event_sender,
            block_applier_run,
            block_applier_thread,
        }
    }
}

impl Actor for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_shutdown(&self.shell_channel, ctx.myself());
    }

    fn post_stop(&mut self) {
        // Set the flag, and let the thread wake up. There is no race condition here, if `unpark`
        // happens first, `park` will return immediately. Hence there is no risk of a deadlock.
        self.block_applier_run.store(false, Ordering::Release);

        let join_handle = self
            .block_applier_thread
            .lock()
            .unwrap()
            .take()
            .expect("Thread join handle is missing");
        join_handle.thread().unpark();
        let _ = join_handle
            .join()
            .expect("Failed to join block applier thread");
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ApplyBlock> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ApplyBlock, _: Sender) {
        if !self.block_applier_run.load(Ordering::Acquire) {
            return;
        }
        self.apply_completed_block(msg, &ctx.system.log());
    }
}

impl Receive<ShellChannelMsg> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        if let ShellChannelMsg::ShuttingDown(_) = msg {
            self.block_applier_run.store(false, Ordering::Release);
            // This event just pings the inner thread to shutdown
            if let Err(e) = self.send_to_queue(Event::ShuttingDown) {
                warn!(ctx.system.log(), "Failed to send ShuttinDown event do internal queue"; "reason" => format!("{:?}", e));
            }
        }
    }
}

/// Possible errors for feeding chain
#[derive(Debug, Fail)]
pub enum FeedChainError {
    #[fail(display = "Cannot resolve current head, no genesis was commited")]
    UnknownCurrentHeadError,
    #[fail(
        display = "Context is not stored, context_hash: {}, reason: {}",
        context_hash, reason
    )]
    MissingContextError {
        context_hash: String,
        reason: String,
    },
    #[fail(display = "Storage read/write error,r eason: {:?}", error)]
    StorageError { error: StorageError },
    #[fail(display = "Protocol service error error, reason: {:?}", error)]
    ProtocolServiceError { error: ProtocolServiceError },
    #[fail(display = "Block apply processing error, reason: {:?}", reason)]
    ProcessingError { reason: String },
}

impl From<StorageError> for FeedChainError {
    fn from(error: StorageError) -> Self {
        FeedChainError::StorageError { error }
    }
}

impl From<ProtocolServiceError> for FeedChainError {
    fn from(error: ProtocolServiceError) -> Self {
        FeedChainError::ProtocolServiceError { error }
    }
}

#[derive(Clone)]
pub(crate) struct BlockApplierThreadSpawner {
    /// actor for managing current head
    chain_current_head_manager: ChainCurrentHeadManagerRef,
    persistent_storage: PersistentStorage,
    init_storage_data: Arc<StorageInitInfo>,
    tezos_env: Arc<TezosEnvironmentConfiguration>,
    tezos_writeable_api: Arc<TezosApiConnectionPool>,
    log: Logger,
}

impl BlockApplierThreadSpawner {
    pub(crate) fn new(
        chain_current_head_manager: ChainCurrentHeadManagerRef,
        persistent_storage: PersistentStorage,
        init_storage_data: Arc<StorageInitInfo>,
        tezos_env: Arc<TezosEnvironmentConfiguration>,
        tezos_writeable_api: Arc<TezosApiConnectionPool>,
        log: Logger,
    ) -> Self {
        Self {
            chain_current_head_manager,
            persistent_storage,
            tezos_writeable_api,
            init_storage_data,
            tezos_env,
            log,
        }
    }

    /// Spawns asynchronous thread, which process events from internal queue
    fn spawn_feeder_thread(
        &self,
    ) -> (
        QueueSender<Event>,
        Arc<AtomicBool>,
        JoinHandle<Result<(), Error>>,
    ) {
        // spawn thread which processes event
        let (block_applier_event_sender, mut block_applier_event_receiver) = channel();
        let block_applier_run = Arc::new(AtomicBool::new(false));

        let block_applier_thread = {
            let chain_current_head_manager = self.chain_current_head_manager.clone();
            let persistent_storage = self.persistent_storage.clone();
            let tezos_writeable_api = self.tezos_writeable_api.clone();
            let init_storage_data = self.init_storage_data.clone();
            let tezos_env = self.tezos_env.clone();
            let log = self.log.clone();
            let block_applier_run = block_applier_run.clone();

            thread::spawn(move || -> Result<(), Error> {
                let block_storage = BlockStorage::new(&persistent_storage);
                let block_meta_storage = BlockMetaStorage::new(&persistent_storage);
                let chain_meta_storage = ChainMetaStorage::new(&persistent_storage);
                let operations_storage = OperationsStorage::new(&persistent_storage);
                let operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);
                let context: Box<dyn ContextApi> = Box::new(TezedgeContext::new(
                    Some(block_storage.clone()),
                    persistent_storage.merkle(),
                ));

                block_applier_run.store(true, Ordering::Release);
                info!(log, "Chain feeder started processing");

                while block_applier_run.load(Ordering::Acquire) {
                    match tezos_writeable_api.pool.get() {
                        Ok(mut protocol_controller) => match feed_chain_to_protocol(
                            &tezos_env,
                            &init_storage_data,
                            &block_applier_run,
                            &chain_current_head_manager,
                            &block_storage,
                            &block_meta_storage,
                            &chain_meta_storage,
                            &operations_storage,
                            &operations_meta_storage,
                            &context,
                            &protocol_controller.api,
                            &mut block_applier_event_receiver,
                            &log,
                        ) {
                            Ok(()) => {
                                protocol_controller.set_release_on_return_to_pool();
                                debug!(log, "Feed chain to protocol finished")
                            }
                            Err(err) => {
                                protocol_controller.set_release_on_return_to_pool();
                                if block_applier_run.load(Ordering::Acquire) {
                                    warn!(log, "Error while feeding chain to protocol"; "reason" => format!("{:?}", err));
                                }
                            }
                        },
                        Err(err) => {
                            warn!(log, "No connection from protocol runner"; "reason" => format!("{:?}", err))
                        }
                    }
                }

                info!(log, "Chain feeder thread finished");
                Ok(())
            })
        };
        (
            block_applier_event_sender,
            block_applier_run,
            block_applier_thread,
        )
    }
}

fn feed_chain_to_protocol(
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: &StorageInitInfo,
    apply_block_run: &AtomicBool,
    chain_current_head_manager: &ChainCurrentHeadManagerRef,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    chain_meta_storage: &ChainMetaStorage,
    operations_storage: &OperationsStorage,
    operations_meta_storage: &OperationsMetaStorage,
    context: &Box<dyn ContextApi>,
    protocol_controller: &ProtocolController,
    block_applier_event_receiver: &mut QueueReceiver<Event>,
    log: &Logger,
) -> Result<(), FeedChainError> {
    // at first we initialize protocol runtime and ffi context
    initialize_protocol_context(
        &apply_block_run,
        chain_current_head_manager,
        block_storage,
        block_meta_storage,
        chain_meta_storage,
        operations_meta_storage,
        context,
        &protocol_controller,
        &log,
        &tezos_env,
        &init_storage_data,
    )?;

    // now just check current head (at least genesis should be there)
    if chain_meta_storage
        .get_current_head(&init_storage_data.chain_id)?
        .is_none()
    {
        // this should not happen here, we applied at least genesis before
        return Err(FeedChainError::UnknownCurrentHeadError);
    };

    // now we can start applying block
    while apply_block_run.load(Ordering::Acquire) {
        // let's handle event, if any
        if let Ok(event) = block_applier_event_receiver.recv() {
            match event {
                Event::ApplyBlock(request) => {
                    // lets apply block batch
                    let ApplyBlock {
                        batch,
                        bootstrapper,
                        chain_id,
                        result_callback,
                        permit,
                    } = request;

                    let mut last_applied: Option<Arc<BlockHash>> = None;
                    let mut condvar_result: Option<Result<(), failure::Error>> = None;

                    // lets apply blocks in order
                    for block_to_apply in batch.take_all_blocks_to_apply() {
                        debug!(log, "Applying block"; "block_header_hash" => block_to_apply.to_base58_check(), "chain_id" => chain_id.to_base58_check(), "sender" => sender_to_string(&bootstrapper));
                        let validated_at_timer = Instant::now();

                        // prepare request and data for block
                        // collect all required data for apply
                        let load_metadata_timer = Instant::now();
                        let apply_block_request_data = prepare_apply_request(
                            &block_to_apply,
                            chain_id.as_ref().clone(),
                            block_storage,
                            block_meta_storage,
                            operations_storage,
                        );
                        let load_metadata_elapsed = load_metadata_timer.elapsed();

                        // apply block and handle result
                        match _apply_block(
                            chain_id.clone(),
                            block_to_apply.clone(),
                            apply_block_request_data,
                            validated_at_timer,
                            load_metadata_elapsed,
                            block_storage,
                            block_meta_storage,
                            context,
                            protocol_controller,
                            init_storage_data.one_context,
                            &log,
                        ) {
                            Ok(result) => {
                                match result {
                                    Some(validated_block) => {
                                        last_applied = Some(block_to_apply);
                                        if result_callback.is_some() {
                                            condvar_result = Some(Ok(()));
                                        }

                                        // notify  chain current head manager (only for new applied block)
                                        chain_current_head_manager.tell(validated_block, None);
                                    }
                                    None => {
                                        last_applied = Some(block_to_apply);
                                        if result_callback.is_some() {
                                            condvar_result = Some(Err(format_err!(
                                                "Block/batch is already applied"
                                            )));
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(log, "Block apply processing failed"; "block" => block_to_apply.to_base58_check(), "reason" => format!("{}", e));

                                // handle condvar immediately
                                if let Err(e) = dispatch_condvar_result(
                                    result_callback.clone(),
                                    || Err(format_err!("{}", e)),
                                    true,
                                ) {
                                    warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                                }
                                if result_callback.is_some() {
                                    // dont process next time
                                    condvar_result = None;
                                }

                                // notify bootstrapper with failed + last_applied
                                if apply_block_run.load(Ordering::Acquire) {
                                    if let Some(bootstrapper) = bootstrapper.as_ref() {
                                        bootstrapper.tell(
                                            BlockBatchApplyFailed {
                                                failed_block: block_to_apply.clone(),
                                            },
                                            None,
                                        );
                                    }
                                }

                                // handle protocol error - continue or restart protocol runner?
                                if let FeedChainError::ProtocolServiceError { error } = e {
                                    handle_protocol_service_error(
                                        error,
                                        |e| warn!(log, "Failed to apply block"; "block" => block_to_apply.to_base58_check(), "reason" => format!("{:?}", e)),
                                    )?;
                                }

                                // just break processing and wait for another event
                                break;
                            }
                        }
                    }

                    // allow others as soon as possible
                    if let Some(permit) = permit {
                        drop(permit);
                    }

                    // notify condvar
                    if let Some(condvar_result) = condvar_result {
                        // notify condvar
                        if let Err(e) =
                            dispatch_condvar_result(result_callback, || condvar_result, true)
                        {
                            warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                        }
                    }

                    // notify after batch success done
                    if apply_block_run.load(Ordering::Acquire) {
                        if let Some(last_applied) = last_applied {
                            // notify bootstrapper just on the end of the success batch
                            if let Some(bootstrapper) = bootstrapper {
                                bootstrapper.tell(BlockBatchApplied { last_applied }, None);
                            }
                        }
                    }
                }
                Event::ShuttingDown => {
                    apply_block_run.store(false, Ordering::Release);
                }
            }
        }
    }

    Ok(())
}

/// Call protocol runner to apply block
///
/// Return ProcessValidatedBlock - if block was applied or None if was already previosly applied else Err
fn _apply_block(
    chain_id: Arc<ChainId>,
    block_hash: Arc<BlockHash>,
    apply_block_request_data: Result<
        (
            ApplyBlockRequest,
            block_meta_storage::Meta,
            Arc<BlockHeaderWithHash>,
        ),
        FeedChainError,
    >,
    validated_at_timer: Instant,
    load_metadata_elapsed: Duration,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    context: &Box<dyn ContextApi>,
    protocol_controller: &ProtocolController,
    one_context: bool,
    log: &Logger,
) -> Result<Option<ProcessValidatedBlock>, FeedChainError> {
    // unwrap result
    let (block_request, mut block_meta, block) = apply_block_request_data?;

    // check if not already applied
    if block_meta.is_applied() {
        debug!(log, "Block is already applied (feeder)"; "block" => block_hash.to_base58_check());
        return Ok(None);
    }

    // try apply block
    let protocol_call_timer = Instant::now();
    let apply_block_result = protocol_controller.apply_block(block_request)?;
    let protocol_call_elapsed = protocol_call_timer.elapsed();
    debug!(log, "Block was applied";
                        "block_header_hash" => block_hash.to_base58_check(),
                        "context_hash" => apply_block_result.context_hash.to_base58_check(),
                        "validation_result_message" => &apply_block_result.validation_result_message);

    if protocol_call_elapsed.gt(&BLOCK_APPLY_DURATION_LONG_TO_LOG) {
        info!(log, "Block was validated with protocol with long processing";
                           "block_header_hash" => block_hash.to_base58_check(),
                           "context_hash" => apply_block_result.context_hash.to_base58_check(),
                           "protocol_call_elapsed" => format!("{:?}", &protocol_call_elapsed));
    }

    // we need to check and wait for context_hash to be 100% sure, that everything is ok
    let context_wait_timer = Instant::now();
    wait_for_context(context, &apply_block_result.context_hash, one_context)?;
    let context_wait_elapsed = context_wait_timer.elapsed();
    if context_wait_elapsed.gt(&CONTEXT_WAIT_DURATION_LONG_TO_LOG) {
        info!(log, "Block was applied with long context processing";
                           "block_header_hash" => block_hash.to_base58_check(),
                           "context_hash" => apply_block_result.context_hash.to_base58_check(),
                           "context_wait_elapsed" => format!("{:?}", &context_wait_elapsed),
                           "protocol_call_elapsed" => format!("{:?}", &protocol_call_elapsed));
    }

    // Lets mark header as applied and store result
    // store success result
    let store_result_timer = Instant::now();
    let _ = store_applied_block_result(
        block_storage,
        block_meta_storage,
        &block_hash,
        apply_block_result,
        &mut block_meta,
    )?;
    let store_result_elapsed = store_result_timer.elapsed();

    Ok(Some(ProcessValidatedBlock::new(
        block,
        chain_id,
        Arc::new(BlockValidationTimer::new(
            validated_at_timer.elapsed(),
            load_metadata_elapsed,
            protocol_call_elapsed,
            context_wait_elapsed,
            store_result_elapsed,
        )),
    )))
}

/// Collects complete data for applying block, if not complete, return None
fn prepare_apply_request(
    block_hash: &BlockHash,
    chain_id: ChainId,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    operations_storage: &OperationsStorage,
) -> Result<
    (
        ApplyBlockRequest,
        block_meta_storage::Meta,
        Arc<BlockHeaderWithHash>,
    ),
    FeedChainError,
> {
    // get block header
    let block = match block_storage.get(block_hash)? {
        Some(block) => Arc::new(block),
        None => {
            return Err(FeedChainError::StorageError {
                error: StorageError::MissingKey,
            });
        }
    };

    // get block_metadata
    let block_meta = match block_meta_storage.get(&block_hash)? {
        Some(meta) => meta,
        None => {
            return Err(FeedChainError::ProcessingError {
                reason: "Block metadata not found".to_string(),
            });
        }
    };

    // get operations
    let operations = operations_storage.get_operations(block_hash)?;

    // get predecessor metadata
    let predecessor = match block_storage.get(&block.header.predecessor())? {
        Some(header) => header,
        None => {
            return Err(FeedChainError::StorageError {
                error: StorageError::MissingKey,
            });
        }
    };

    // predecessor additional data
    let (
        predecessor_block_metadata_hash,
        predecessor_ops_metadata_hash,
        predecessor_max_operations_ttl,
    ) = match block_meta_storage.get_additional_data(&block.header.predecessor())? {
        Some(predecessor_additional_data) => predecessor_additional_data.into(),
        None => {
            return Err(FeedChainError::StorageError {
                error: StorageError::MissingKey,
            });
        }
    };

    Ok((
        ApplyBlockRequest {
            chain_id,
            block_header: block.header.as_ref().clone(),
            pred_header: predecessor.header.as_ref().clone(),
            operations: ApplyBlockRequest::convert_operations(operations),
            max_operations_ttl: predecessor_max_operations_ttl as i32,
            predecessor_block_metadata_hash,
            predecessor_ops_metadata_hash,
        },
        block_meta,
        block,
    ))
}

/// This initializes ocaml runtime and protocol context,
/// if we start with new databazes without genesis,
/// it ensures correct initialization of storage with genesis and his data.
pub(crate) fn initialize_protocol_context(
    apply_block_run: &AtomicBool,
    chain_current_head_manager: &ChainCurrentHeadManagerRef,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    chain_meta_storage: &ChainMetaStorage,
    operations_meta_storage: &OperationsMetaStorage,
    context: &Box<dyn ContextApi>,
    protocol_controller: &ProtocolController,
    log: &Logger,
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: &StorageInitInfo,
) -> Result<(), FeedChainError> {
    let validated_at_timer = Instant::now();

    // we must check if genesis is applied, if not then we need "commit_genesis" to context
    let load_metadata_timer = Instant::now();
    let need_commit_genesis =
        match block_meta_storage.get(&init_storage_data.genesis_block_header_hash)? {
            Some(genesis_meta) => !genesis_meta.is_applied(),
            None => true,
        };
    let load_metadata_elapsed = load_metadata_timer.elapsed();
    trace!(log, "Looking for genesis if applied"; "need_commit_genesis" => need_commit_genesis);

    // initialize protocol context runtime
    let protocol_call_timer = Instant::now();
    let context_init_info = protocol_controller
        .init_protocol_for_write(need_commit_genesis, &init_storage_data.patch_context)?;
    let protocol_call_elapsed = protocol_call_timer.elapsed();
    info!(log, "Protocol context initialized"; "context_init_info" => format!("{:?}", &context_init_info), "need_commit_genesis" => need_commit_genesis);

    if need_commit_genesis {
        // if we needed commit_genesis, it means, that it is apply of 0 block,
        // which initiates genesis protocol in context, so we need to store some data, like we do in normal apply, see below store_apply_block_result
        if let Some(genesis_context_hash) = context_init_info.genesis_commit_hash {
            // at first store genesis to storage
            let store_result_timer = Instant::now();
            let genesis_with_hash = initialize_storage_with_genesis_block(
                block_storage,
                block_meta_storage,
                &init_storage_data,
                &tezos_env,
                &genesis_context_hash,
                &log,
            )?;

            let context_wait_timer = Instant::now();
            wait_for_context(
                context,
                &genesis_context_hash,
                init_storage_data.one_context,
            )?;
            let context_wait_elapsed = context_wait_timer.elapsed();

            // call get additional/json data for genesis (this must be second call, because this triggers context.checkout)
            // this needs to be second step, because, this triggers context.checkout, so we need to call it after store_commit_genesis_result
            let commit_data = protocol_controller.genesis_result_data(&genesis_context_hash)?;

            // this, marks genesis block as applied
            let _ = store_commit_genesis_result(
                block_storage,
                block_meta_storage,
                chain_meta_storage,
                operations_meta_storage,
                &init_storage_data,
                commit_data,
            )?;
            let store_result_elapsed = store_result_timer.elapsed();

            // notify listeners
            if apply_block_run.load(Ordering::Acquire) {
                // notify others that the block successfully applied
                chain_current_head_manager.tell(
                    ProcessValidatedBlock::new(
                        Arc::new(genesis_with_hash),
                        Arc::new(init_storage_data.chain_id.clone()),
                        Arc::new(BlockValidationTimer::new(
                            validated_at_timer.elapsed(),
                            load_metadata_elapsed,
                            protocol_call_elapsed,
                            context_wait_elapsed,
                            store_result_elapsed,
                        )),
                    ),
                    None,
                );
            }
        }
    }

    Ok(())
}

fn sender_to_string(sender: &Option<PeerBranchBootstrapperRef>) -> String {
    match sender {
        Some(sender) => format!("{}-{}", sender.name(), sender.uri().to_string()),
        None => "--none--".to_string(),
    }
}

const CONTEXT_WAIT_DURATION: (Duration, Duration) =
    (Duration::from_secs(60 * 60 * 2), Duration::from_millis(15));
const CONTEXT_WAIT_DURATION_LONG_TO_LOG: Duration = Duration::from_secs(30);
const BLOCK_APPLY_DURATION_LONG_TO_LOG: Duration = Duration::from_secs(30);

/// Context_listener is now asynchronous, so we need to make sure, that it is processed, so we wait a little bit
pub fn wait_for_context(
    context: &Box<dyn ContextApi>,
    context_hash: &ContextHash,
    one_context: bool,
) -> Result<(), FeedChainError> {
    if one_context {
        return Ok(());
    }
    let (timeout, delay): (Duration, Duration) = CONTEXT_WAIT_DURATION;
    let start = SystemTime::now();

    // try find context_hash
    loop {
        // if success, than ok
        if let Ok(true) = context.is_committed(context_hash) {
            break Ok(());
        }

        // kind of simple retry policy
        let elapsed = start
            .elapsed()
            .map_err(|e| FeedChainError::MissingContextError {
                reason: format!("{}", e),
                context_hash: context_hash.to_base58_check(),
            })?;
        if elapsed.le(&timeout) {
            thread::sleep(delay);
        } else {
            break Err(FeedChainError::MissingContextError {
                reason: format!("Block inject - context was not processed for context_hash: {}, timeout (timeout: {:?}, delay: {:?})", context_hash.to_base58_check(), timeout, delay),
                context_hash: context_hash.to_base58_check(),
            });
        }
    }
}
