// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Sends blocks to the `protocol_runner`.
//! This actor is responsible for correct applying of blocks with Tezos protocol in context
//! This actor is aslo responsible for correct initialization of genesis in storage.

use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver as QueueReceiver, Sender as QueueSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{format_err, Error};
use slog::{debug, info, trace, warn, Logger};
use tezedge_actor_system::actors::*;
use tezos_protocol_ipc_client::{
    ProtocolRunnerApi, ProtocolRunnerConnection, ProtocolServiceError,
};
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId};
use shell_integration::{
    dispatch_oneshot_result, InjectBlockError, InjectBlockOneshotResultCallback,
    InjectBlockOneshotResultCallbackResult, OneshotResultCallback, RetryPolicy,
    ThreadRunningStatus, ThreadWatcher,
};
use storage::{
    block_meta_storage, BlockAdditionalData, BlockHeaderWithHash, BlockMetaStorageReader,
    CycleErasStorage, CycleMetaStorage, PersistentStorage,
};
use storage::{
    initialize_storage_with_genesis_block, store_applied_block_result, store_commit_genesis_result,
    BlockMetaStorage, BlockStorage, BlockStorageReader, ChainMetaStorage, ConstantsStorage,
    OperationsMetaStorage, OperationsStorage, OperationsStorageReader, StorageError,
    StorageInitInfo,
};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::ApplyBlockRequest;

use crate::chain_manager::{ChainManagerRef, ProcessValidatedBlock};
use crate::peer_branch_bootstrapper::PeerBranchBootstrapperRef;
use crate::state::ApplyBlockBatch;
use crate::stats::apply_block_stats::{ApplyBlockStats, BlockValidationTimer};
use tokio::sync::OwnedSemaphorePermit;

pub type InitializeContextOneshotResultCallback =
    OneshotResultCallback<Result<(), InitializeContextOneshotResultCallbackError>>;

#[derive(Debug)]
pub struct InitializeContextOneshotResultCallbackError {
    #[allow(dead_code)]
    reason: String,
}

/// How often to print stats in logs
const LOG_INTERVAL: Duration = Duration::from_secs(60);

/// For restarting scenario, retry policy max retries for one block
const DEFAULT_MAX_RETRIES: u8 = 2;

/// Message commands [`ChainFeeder`] to apply completed block.
#[derive(Clone, Debug)]
pub struct ApplyBlock {
    batch: ApplyBlockBatch,
    chain_id: Arc<ChainId>,
    chain_manager: Arc<ChainManagerRef>,
    bootstrapper: Option<PeerBranchBootstrapperRef>,
    /// Callback can be used to wait for apply block result
    result_callback: Option<InjectBlockOneshotResultCallback>,

    /// Simple lock guard, for easy synchronization
    permit: Option<Arc<ApplyBlockPermit>>,

    /// Retry policy - used for restarting scenario
    retry_policy: Option<RetryPolicy<Arc<BlockHash>>>,
}

impl ApplyBlock {
    pub fn new(
        chain_id: Arc<ChainId>,
        batch: ApplyBlockBatch,
        chain_manager: Arc<ChainManagerRef>,
        result_callback: Option<InjectBlockOneshotResultCallback>,
        bootstrapper: Option<PeerBranchBootstrapperRef>,
        permit: Option<ApplyBlockPermit>,
    ) -> Self {
        Self {
            chain_id,
            chain_manager,
            batch,
            result_callback,
            bootstrapper,
            permit: permit.map(Arc::new),
            retry_policy: None,
        }
    }
}

/// Used for synchronization, if caller want to limit or need to know, if previous request was done or not
pub type ApplyBlockPermit = OwnedSemaphorePermit;

/// Event is fired, when some batch was finished, so next can go
#[derive(Clone, Debug)]
pub struct ApplyBlockDone {
    pub last_applied: Arc<BlockHash>,

    /// Simple lock guard, for easy synchronization
    pub permit: Option<Arc<ApplyBlockPermit>>,
}

/// Event is fired, when some batch was not applied and error occured
#[derive(Clone, Debug)]
pub struct ApplyBlockFailed {
    pub failed_block: Arc<BlockHash>,

    /// Simple lock guard, for easy synchronization
    pub permit: Option<Arc<ApplyBlockPermit>>,
}

/// Message commands [`ChainFeeder`] to log its internal stats.
#[derive(Clone, Debug)]
pub struct LogStats;

/// Message tells [`ChainFeeder`] to update apply block stats
#[derive(Clone, Debug)]
pub struct UpdateApplyBlockStats {
    stats: ApplyBlockStats,
}

/// Internal queue commands
pub(crate) enum Event {
    ApplyBlock(ApplyBlock, ChainFeederRef),
    ShuttingDown,
}

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(ApplyBlock, LogStats, UpdateApplyBlockStats)]
pub struct ChainFeeder {
    /// Internal queue sender
    block_applier_event_sender: Arc<Mutex<QueueSender<Event>>>,
    /// Thread where blocks are applied will run until this is set to `false`
    block_applier_thread_run: ThreadRunningStatus,

    /// Statistics for applying blocks
    apply_block_stats: ApplyBlockStats,
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
        persistent_storage: PersistentStorage,
        tezos_protocol_api: Arc<ProtocolRunnerApi>,
        init_storage_data: StorageInitInfo,
        tezos_env: TezosEnvironmentConfiguration,
        log: Logger,
        initialize_context_result_callback: InitializeContextOneshotResultCallback,
    ) -> Result<(ChainFeederRef, ThreadWatcher), CreateError> {
        let tokio_runtime = tezos_protocol_api.tokio_runtime.clone();
        // spawn inner thread
        let (block_applier_event_sender, block_applier_thread_watcher) =
            BlockApplierThreadSpawner::new(
                persistent_storage,
                Arc::new(init_storage_data),
                Arc::new(tezos_env),
                tezos_protocol_api,
                log.clone(),
            )
            .spawn_feeder_thread(
                "chain-feedr-ctx".into(),
                initialize_context_result_callback,
                &tokio_runtime,
            )
            .map_err(|e| {
                warn!(log, "Failed to spawn chain feeder thread"; "reason" => format!("{}", e));
                CreateError::Panicked
            })?;

        sys.actor_of_props::<ChainFeeder>(
            ChainFeeder::name(),
            Props::new_args((
                Arc::new(Mutex::new(block_applier_event_sender)),
                block_applier_thread_watcher.thread_running_status().clone(),
            )),
        )
        .map(|actor| (actor, block_applier_thread_watcher))
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

    fn apply_completed_block(&self, msg: ApplyBlock, chain_feeder: ChainFeederRef, log: &Logger) {
        // add request to queue
        let result_callback = msg.result_callback.clone();
        if let Err(e) = self.send_to_queue(Event::ApplyBlock(msg, chain_feeder.clone())) {
            warn!(log, "Failed to send `apply block request` to queue"; "reason" => format!("{}", e));
            if let Err(de) = dispatch_oneshot_result(result_callback, || {
                Err(InjectBlockError {
                    reason: format!("{}", e),
                })
            }) {
                warn!(log, "Failed to dispatch result"; "reason" => format!("{}", de));
            }

            // just ping chain_feeder
            chain_feeder.tell(
                UpdateApplyBlockStats {
                    stats: ApplyBlockStats::default(),
                },
                None,
            );
        }
    }

    fn update_stats(&mut self, new_stats: ApplyBlockStats) {
        self.apply_block_stats.merge(new_stats);
    }
}

impl ActorFactoryArgs<(Arc<Mutex<QueueSender<Event>>>, ThreadRunningStatus)> for ChainFeeder {
    fn create_args(
        (block_applier_event_sender, block_applier_thread_run): (
            Arc<Mutex<QueueSender<Event>>>,
            ThreadRunningStatus,
        ),
    ) -> Self {
        ChainFeeder {
            block_applier_event_sender,
            block_applier_thread_run,
            apply_block_stats: ApplyBlockStats::default(),
        }
    }
}

impl Actor for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        ctx.schedule::<Self::Msg, _>(
            LOG_INTERVAL / 2,
            LOG_INTERVAL,
            ctx.myself(),
            None,
            LogStats.into(),
        );
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ApplyBlock> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ApplyBlock, _: Sender) {
        // do not process any message, when thread is down
        if !self.block_applier_thread_run.load(Ordering::Acquire) {
            return;
        }
        self.apply_completed_block(msg, ctx.myself(), &ctx.system.log());
    }
}

impl Receive<UpdateApplyBlockStats> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, msg: UpdateApplyBlockStats, _: Sender) {
        // do not process any message, when thread is down
        if !self.block_applier_thread_run.load(Ordering::Acquire) {
            return;
        }
        self.update_stats(msg.stats);
    }
}

impl Receive<LogStats> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _: LogStats, _: Sender) {
        let log = ctx.system.log();

        // calculate applied stats
        let (last_applied, last_applied_block_level, last_applied_block_elapsed_in_secs) = {
            let applied_block_lasts_count = self.apply_block_stats.applied_block_lasts_count();

            if *applied_block_lasts_count > 0 {
                let validation = self.apply_block_stats.print_formatted_average_times();

                // collect stats before clearing
                let stats = format!(
                    "({} blocks validated in time: {:?}, average times [{}]",
                    applied_block_lasts_count,
                    self.apply_block_stats.sum_validated_at_time(),
                    validation,
                );
                let applied_block_level = *self.apply_block_stats.applied_block_level();
                let applied_block_last = self
                    .apply_block_stats
                    .applied_block_last()
                    .map(|i| i.elapsed().as_secs());

                // clear stats for next run
                self.apply_block_stats.clear_applied_block_lasts();

                (stats, applied_block_level, applied_block_last)
            } else {
                (
                    format!("({} blocks)", applied_block_lasts_count),
                    None,
                    None,
                )
            }
        };

        info!(log, "Blocks apply info";
            "last_applied" => last_applied,
            "last_applied_batch_block_level" => last_applied_block_level,
            "last_applied_batch_block_elapsed_in_secs" => last_applied_block_elapsed_in_secs);
    }
}

/// Possible errors for feeding chain
#[derive(Debug, Error)]
pub enum FeedChainError {
    #[error("{reason:?}")]
    InitializeContextError { reason: InitializeContextError },
    #[error("{reason:?}")]
    RequiredContextRestartError { reason: RequiredContextRestartError },
}

impl From<InitializeContextError> for FeedChainError {
    fn from(reason: InitializeContextError) -> Self {
        FeedChainError::InitializeContextError { reason }
    }
}

impl From<RequiredContextRestartError> for FeedChainError {
    fn from(reason: RequiredContextRestartError) -> Self {
        FeedChainError::RequiredContextRestartError { reason }
    }
}

#[derive(Debug, Error)]
#[error("Detected request for restarting context, reason: {reason}")]
pub struct RequiredContextRestartError {
    reason: String,
    /// We have possibility, to retry block application, in case of some errors, after restarting
    apply_block_request_to_retry: Option<(ApplyBlock, ChainFeederRef)>,
}

#[derive(Debug, Error)]
pub enum InitializeContextError {
    #[error("Failed to initialize context because of storage error, reason: {reason}")]
    StorageError { reason: StorageError },
    #[error("Failed to initialize context because of protocol error, reason: {reason}")]
    ProtocolServiceError { reason: ProtocolServiceError },
}

impl From<StorageError> for InitializeContextError {
    fn from(reason: StorageError) -> Self {
        InitializeContextError::StorageError { reason }
    }
}

impl From<ProtocolServiceError> for InitializeContextError {
    fn from(reason: ProtocolServiceError) -> Self {
        InitializeContextError::ProtocolServiceError { reason }
    }
}

#[derive(Debug, Error)]
pub enum ApplyBlockBatchError {
    #[error("Storage read/write error, reason: {reason:?}")]
    StorageError { reason: StorageError },
    #[error("Protocol service error error, reason: {reason:?}")]
    ProtocolServiceError { reason: ProtocolServiceError },
}

impl From<StorageError> for ApplyBlockBatchError {
    fn from(reason: StorageError) -> Self {
        ApplyBlockBatchError::StorageError { reason }
    }
}

impl From<ProtocolServiceError> for ApplyBlockBatchError {
    fn from(reason: ProtocolServiceError) -> Self {
        ApplyBlockBatchError::ProtocolServiceError { reason }
    }
}

#[derive(Clone)]
pub(crate) struct BlockApplierThreadSpawner {
    persistent_storage: PersistentStorage,
    init_storage_data: Arc<StorageInitInfo>,
    tezos_env: Arc<TezosEnvironmentConfiguration>,
    tezos_protocol_api: Arc<ProtocolRunnerApi>,
    log: Logger,
}

impl BlockApplierThreadSpawner {
    pub(crate) fn new(
        persistent_storage: PersistentStorage,
        init_storage_data: Arc<StorageInitInfo>,
        tezos_env: Arc<TezosEnvironmentConfiguration>,
        tezos_protocol_api: Arc<ProtocolRunnerApi>,
        log: Logger,
    ) -> Self {
        Self {
            persistent_storage,
            tezos_protocol_api,
            init_storage_data,
            tezos_env,
            log,
        }
    }

    /// Spawns asynchronous thread, which process events from internal queue
    fn spawn_feeder_thread(
        &self,
        thread_name: String,
        initialize_context_result_callback: InitializeContextOneshotResultCallback,
        tokio_runtime: &tokio::runtime::Handle,
    ) -> Result<(QueueSender<Event>, ThreadWatcher), anyhow::Error> {
        // spawn thread which processes event
        let (block_applier_event_sender, mut block_applier_event_receiver) = channel();
        let mut block_applier_thread_watcher = {
            let block_applier_event_sender = block_applier_event_sender.clone();
            ThreadWatcher::start(
                thread_name.clone(),
                Box::new(move || {
                    block_applier_event_sender
                        .send(Event::ShuttingDown)
                        .map_err(|e| e.into())
                }),
            )
        };

        let block_applier_thread = {
            let persistent_storage = self.persistent_storage.clone();
            let tezos_protocol_api = Arc::clone(&self.tezos_protocol_api);
            let tokio_runtime = tokio_runtime.clone();
            let init_storage_data = self.init_storage_data.clone();
            let tezos_env = self.tezos_env.clone();
            let log = self.log.clone();
            let apply_block_run = block_applier_thread_watcher.thread_running_status().clone();
            let mut initialize_context_result_callback = Some(initialize_context_result_callback);

            thread::Builder::new().name(thread_name).spawn(move || {
                info!(log, "Chain feeder thread starting");
                let block_storage = BlockStorage::new(&persistent_storage);
                let block_meta_storage = BlockMetaStorage::new(&persistent_storage);
                let chain_meta_storage = ChainMetaStorage::new(&persistent_storage);
                let operations_storage = OperationsStorage::new(&persistent_storage);
                let operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);
                let cycle_meta_storage = CycleMetaStorage::new(&persistent_storage);
                let cycle_eras_storage = CycleErasStorage::new(&persistent_storage);
                let constants_storage = ConstantsStorage::new(&persistent_storage);

                // restart/retry feature
                let mut apply_block_request_to_retry: Option<(ApplyBlock, ChainFeederRef)> = None;

                while apply_block_run.load(Ordering::Acquire) {
                    info!(log, "Chain feeding starting");
                    match tezos_protocol_api.writable_connection_sync() {
                        Ok(mut protocol_controller) => match feed_chain_to_protocol(
                            &tezos_env,
                            &init_storage_data,
                            &apply_block_run,
                            &block_storage,
                            &block_meta_storage,
                            &chain_meta_storage,
                            &operations_storage,
                            &operations_meta_storage,
                            &cycle_meta_storage,
                            &cycle_eras_storage,
                            &constants_storage,
                            &mut protocol_controller,
                            &tokio_runtime,
                            &mut block_applier_event_receiver,
                            &mut initialize_context_result_callback,
                            &mut apply_block_request_to_retry,
                            &log,
                        ) {
                            Ok(()) => {
                                debug!(log, "Feed chain to protocol finished")
                            }
                            Err(err) => {
                                if apply_block_run.load(Ordering::Acquire) {
                                    match &err {
                                        FeedChainError::InitializeContextError {reason} => {
                                            warn!(log, "Failed to intialize context and continue feeding chain to protocol"; "reason" => format!("{:?}", reason));
                                        }
                                        FeedChainError::RequiredContextRestartError {reason} => {
                                            warn!(log, "Error while feeding chain to protocol"; "reason" => &reason.reason, "has_apply_block_request_to_retry" => reason.apply_block_request_to_retry.is_some());
                                        }
                                    }
                                }

                                // handle possible retry
                                if let FeedChainError::RequiredContextRestartError {reason} = err {
                                    apply_block_request_to_retry = reason.apply_block_request_to_retry;
                                }
                            }
                        },
                        Err(err) => {
                            warn!(log, "No connection from protocol runner"; "reason" => format!("{:?}", err));
                            panic!();
                        }
                    }
                }

                info!(log, "Chain feeder thread finished");
            })?
        };
        block_applier_thread_watcher.set_thread(block_applier_thread);
        Ok((block_applier_event_sender, block_applier_thread_watcher))
    }
}

#[inline]
fn feed_chain_to_protocol(
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: &StorageInitInfo,
    apply_block_run: &ThreadRunningStatus,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    chain_meta_storage: &ChainMetaStorage,
    operations_storage: &OperationsStorage,
    operations_meta_storage: &OperationsMetaStorage,
    cycle_meta_storage: &CycleMetaStorage,
    cycle_eras_storage: &CycleErasStorage,
    constants_storage: &ConstantsStorage,
    protocol_controller: &mut ProtocolRunnerConnection,
    tokio_runtime: &tokio::runtime::Handle,
    block_applier_event_receiver: &mut QueueReceiver<Event>,
    initialize_context_result_callback: &mut Option<InitializeContextOneshotResultCallback>,
    apply_block_request_to_retry: &mut Option<(ApplyBlock, ChainFeederRef)>,
    log: &Logger,
) -> Result<(), FeedChainError> {
    // at first we initialize protocol runtime and ffi context
    match initialize_protocol_context(
        block_storage,
        block_meta_storage,
        chain_meta_storage,
        operations_meta_storage,
        protocol_controller,
        tokio_runtime,
        log,
        tezos_env,
        init_storage_data,
    ) {
        Ok(_) => {
            // if we came here, everything is ok, and context is initialized ok
            if let Some(initialize_context_result_callback) =
                initialize_context_result_callback.take()
            {
                let _ =
                    dispatch_oneshot_result(Some(initialize_context_result_callback), || Ok(()));
            }
        }
        Err(e) => {
            if let Some(initialize_context_result_callback) =
                initialize_context_result_callback.take()
            {
                let _ = dispatch_oneshot_result(Some(initialize_context_result_callback), || {
                    Err(InitializeContextOneshotResultCallbackError {
                        reason: format!("{}", e),
                    })
                });
            }
            return Err(e.into());
        }
    }

    let replay_mode = init_storage_data.replay.is_some();

    // lets retry a last batch before restart
    if let Some((request, chain_feeder)) = apply_block_request_to_retry.take() {
        info!(log, "Retrying block application"; "from_block" => request.batch.block_to_apply.to_base58_check(), "successors_count" => request.batch.successors_size(), "retry_policy" => format!("{:?}", request.retry_policy));
        _handle_apply_block_request(
            request,
            chain_feeder,
            block_storage,
            block_meta_storage,
            operations_storage,
            cycle_meta_storage,
            cycle_eras_storage,
            constants_storage,
            protocol_controller,
            replay_mode,
            apply_block_run,
            tokio_runtime,
            log,
        )?;
    }

    // now we can start and listen for new applying block requests from queue
    while apply_block_run.load(Ordering::Acquire) {
        // let's handle event, if any
        if let Ok(event) = block_applier_event_receiver.recv() {
            match event {
                Event::ApplyBlock(request, chain_feeder) => {
                    _handle_apply_block_request(
                        request,
                        chain_feeder,
                        block_storage,
                        block_meta_storage,
                        operations_storage,
                        cycle_meta_storage,
                        cycle_eras_storage,
                        constants_storage,
                        protocol_controller,
                        replay_mode,
                        apply_block_run,
                        tokio_runtime,
                        log,
                    )?;
                }
                Event::ShuttingDown => {
                    // just finish the loop
                    info!(
                        log,
                        "Chain feeder thread worker received shutting down event"
                    );
                    break;
                }
            }
        }
    }

    Ok(())
}

#[inline]
fn _handle_apply_block_request(
    request: ApplyBlock,
    chain_feeder: ChainFeederRef,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    operations_storage: &OperationsStorage,
    cycle_meta_storage: &CycleMetaStorage,
    cycle_eras_storage: &CycleErasStorage,
    constants_storage: &ConstantsStorage,
    protocol_controller: &mut ProtocolRunnerConnection,
    replay_mode: bool,
    apply_block_run: &ThreadRunningStatus,
    tokio_runtime: &tokio::runtime::Handle,
    log: &Logger,
) -> Result<(), RequiredContextRestartError> {
    // lets apply block batch
    let ApplyBlock {
        batch,
        bootstrapper,
        chain_manager,
        chain_id,
        result_callback,
        permit,
        retry_policy,
    } = request;

    // callback for notify chain_feeder about new stats
    let notify_stats = |mut stats: Option<ApplyBlockStats>| {
        if apply_block_run.load(Ordering::Acquire) {
            // fire stats
            if let Some(stats) = stats.take() {
                chain_feeder.tell(UpdateApplyBlockStats { stats }, None);
            }
        }
    };
    // callback for notify bootstrapper about some successfulyl last_applied block
    let notify_last_applied =
        |last_applied: Option<Arc<BlockHash>>,
         apply_block_permit: Option<Arc<ApplyBlockPermit>>| {
            if apply_block_run.load(Ordering::Acquire) {
                if let Some(last_applied) = last_applied {
                    // notify bootstrapper just on the end of the success batch
                    if let Some(bootstrapper) = bootstrapper.as_ref() {
                        bootstrapper.tell(
                            ApplyBlockDone {
                                last_applied,
                                permit: apply_block_permit,
                            },
                            None,
                        );
                    }
                }
            }
        };
    // callback for notify bootstrapper about some failed batch block
    let notify_last_failed =
        |last_failed: Arc<BlockHash>, apply_block_permit: Option<Arc<ApplyBlockPermit>>| {
            if apply_block_run.load(Ordering::Acquire) {
                // notify bootstrapper just on the end of the success batch
                if let Some(bootstrapper) = bootstrapper.as_ref() {
                    // TODO: posunut sem aj zvysok batchu, ze nebude naaplikovany
                    bootstrapper.tell(
                        ApplyBlockFailed {
                            failed_block: last_failed,
                            permit: apply_block_permit,
                        },
                        None,
                    );
                }
            }
        };
    // callback for notify condvar
    let notify_oneshot_result =
        |oneshot_callback: Option<InjectBlockOneshotResultCallback>,
         oneshot_result: InjectBlockOneshotResultCallbackResult| {
            // notify condvar, if some result
            if oneshot_callback.is_some() {
                if let Err(e) = dispatch_oneshot_result(oneshot_callback, || oneshot_result) {
                    warn!(log, "Failed to dispatch result (chain_feeder)"; "reason" => format!("{}", e));
                }
            }
        };

    let mut last_applied: Option<Arc<BlockHash>> = None;
    let mut batch_stats = Some(ApplyBlockStats::default());
    let mut oneshot_result: Option<InjectBlockOneshotResultCallbackResult> = None;
    let mut previous_block_data_cache: Option<(Arc<BlockHeaderWithHash>, BlockAdditionalData)> =
        None;

    // lets apply blocks in order
    let mut batch = batch.take_all_blocks_to_apply();
    while let Some(block_to_apply) = batch.pop_front() {
        debug!(log, "Applying block"; "block_header_hash" => block_to_apply.to_base58_check(), "chain_id" => chain_id.to_base58_check());

        if !apply_block_run.load(Ordering::Acquire) {
            info!(log, "Shutdown detected, so stopping block batch apply immediately";
                       "block_header_hash" => block_to_apply.to_base58_check(), "chain_id" => chain_id.to_base58_check());
            return Ok(());
        }

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
            previous_block_data_cache,
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
            cycle_meta_storage,
            cycle_eras_storage,
            constants_storage,
            protocol_controller,
            replay_mode,
            tokio_runtime,
            log,
        ) {
            Ok(result) => {
                match result {
                    Some((validated_block, block_additional_data, block_validation_timer)) => {
                        last_applied = Some(block_to_apply);
                        if result_callback.is_some() {
                            oneshot_result = Some(Ok(()));
                        }
                        previous_block_data_cache =
                            Some((validated_block.block.clone(), block_additional_data));

                        // update state
                        if let Some(stats) = batch_stats.as_mut() {
                            stats.set_applied_block_level(validated_block.block.header.level());
                            stats.add_block_validation_stats(&block_validation_timer);
                        }

                        // notify  chain manager (only for new applied block)
                        chain_manager.tell(validated_block, None);
                    }
                    None => {
                        last_applied = Some(block_to_apply);
                        if result_callback.is_some() {
                            oneshot_result = Some(Err(InjectBlockError {
                                reason: "Block/batch is already applied".to_string(),
                            }));
                        }
                        previous_block_data_cache = None;
                    }
                }
            }
            Err(e) => {
                warn!(log, "Block apply processing failed"; "block" => block_to_apply.to_base58_check(), "reason" => format!("{}", e));

                // now we need to analyse error:
                // 1. or if protocol_runner just failed (OOM killer, some unexpected ipc error, ...) and restart could be enougth
                // 2. or if we need to stop the batch processing and report wrong batch without restarting
                let need_to_restart_context =
                    matches!(&e, ApplyBlockBatchError::ProtocolServiceError { .. });

                // callback for handling failed batch
                let handle_as_failed =
                    |oneshot_callback: Option<InjectBlockOneshotResultCallback>,
                     stats: Option<ApplyBlockStats>,
                     last_applied_block: Option<Arc<BlockHash>>,
                     last_failed_block: Arc<BlockHash>,
                     apply_block_permit: Option<Arc<ApplyBlockPermit>>,
                     error: ApplyBlockBatchError| {
                        notify_oneshot_result(
                            oneshot_callback,
                            Err(InjectBlockError {
                                reason: format!("{}", error),
                            }),
                        );
                        notify_stats(stats);
                        notify_last_applied(last_applied_block, None);
                        notify_last_failed(last_failed_block, apply_block_permit);
                    };

                if need_to_restart_context {
                    // we need to restart, but we handle some stuff at first

                    // reschedule the rest, if possible, or finish batch
                    let retry_policy = match retry_policy {
                        Some(retry_policy) => {
                            // handle actual block, if we could retry it or fail
                            retry_policy.next_retry(&block_to_apply)
                        }
                        None => {
                            // block was not retried before, so we allow to retry
                            Some(RetryPolicy::new(
                                block_to_apply.clone(),
                                DEFAULT_MAX_RETRIES,
                            ))
                        }
                    };
                    let restart_reason = format!("{:?}", e);
                    let apply_block_request_to_retry = if retry_policy.is_some() {
                        info!(log, "Preparing for retry block application"; "from_block" => block_to_apply.to_base58_check(), "successors_count" => batch.len(), "retry_policy" => format!("{:?}", retry_policy));
                        notify_stats(batch_stats);
                        notify_last_applied(last_applied, None);

                        // reschedule batch with retry policy
                        let apply_block_request_to_retry = ApplyBlock {
                            chain_id,
                            batch: ApplyBlockBatch::batch(block_to_apply, batch),
                            chain_manager,
                            result_callback,
                            bootstrapper,
                            permit,
                            retry_policy,
                        };
                        Some((apply_block_request_to_retry, chain_feeder))
                    } else {
                        // here we cannot do anything, we cannot retry, just to propagate error and stop batch
                        warn!(log, "Block apply processing failed, no more retries"; "block" => block_to_apply.to_base58_check(), "reason" => format!("{}", e));
                        handle_as_failed(
                            result_callback,
                            batch_stats,
                            last_applied,
                            block_to_apply,
                            permit,
                            e,
                        );
                        None
                    };

                    // now throw error to restart and process optional retry
                    return Err(RequiredContextRestartError {
                        reason: restart_reason,
                        apply_block_request_to_retry,
                    });
                } else {
                    // we dont need to rest, so we just stop the batch processing
                    handle_as_failed(
                        result_callback,
                        batch_stats,
                        last_applied,
                        block_to_apply,
                        permit,
                        e,
                    );

                    // just break processing and wait for another event - nothing more to do
                    return Ok(());
                }
            }
        }
    }

    // now handle successfull batch application
    // notify other actors after batch success done
    if let Some(oneshot_result) = oneshot_result {
        notify_oneshot_result(result_callback, oneshot_result);
    }
    notify_stats(batch_stats);
    notify_last_applied(last_applied, permit);

    // if we came here, everything is ok
    Ok(())
}

/// Call protocol runner to apply block
///
/// Return ProcessValidatedBlock - if block was applied or None if was already previosly applied else Err
#[inline]
fn _apply_block(
    chain_id: Arc<ChainId>,
    block_hash: Arc<BlockHash>,
    apply_block_request_data: Result<
        (
            ApplyBlockRequest,
            block_meta_storage::Meta,
            Arc<BlockHeaderWithHash>,
        ),
        StorageError,
    >,
    validated_at_timer: Instant,
    load_metadata_elapsed: Duration,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    cycle_meta_storage: &CycleMetaStorage,
    cycle_eras_storage: &CycleErasStorage,
    constants_storage: &ConstantsStorage,
    protocol_controller: &mut ProtocolRunnerConnection,
    replay_mode: bool,
    tokio_runtime: &tokio::runtime::Handle,
    log: &Logger,
) -> Result<
    Option<(
        ProcessValidatedBlock,
        BlockAdditionalData,
        BlockValidationTimer,
    )>,
    ApplyBlockBatchError,
> {
    // unwrap result
    let (block_request, mut block_meta, block) = apply_block_request_data?;

    // check if not already applied
    if block_meta.is_applied() && !replay_mode {
        info!(log, "Block is already applied (feeder)";
                   "block_header_hash" => block_hash.to_base58_check(),
                   "block_header_level" => block.header.level());
        return Ok(None);
    }

    // try apply block
    let protocol_call_timer = Instant::now();
    let apply_block_result = tokio::task::block_in_place(|| {
        tokio_runtime.block_on(protocol_controller.apply_block(block_request))
    })?;
    let protocol_call_elapsed = protocol_call_timer.elapsed();

    if !apply_block_result.cycle_rolls_owner_snapshots.is_empty() {
        debug!(
            log,
            "Block application returned {} new snapshots",
            apply_block_result.cycle_rolls_owner_snapshots.len()
        );
    }

    if let Some(json) = &apply_block_result.new_protocol_constants_json {
        debug!(log, "Block application returned new constants: {}", json,);
    }

    debug!(log, "Block was applied";
           "block_header_hash" => block_hash.to_base58_check(),
           "block_header_level" => block.header.level(),
           "context_hash" => apply_block_result.context_hash.to_base58_check(),
           "validation_result_message" => &apply_block_result.validation_result_message);

    if protocol_call_elapsed.gt(&BLOCK_APPLY_DURATION_LONG_TO_LOG) {
        let commit_time_duration = Duration::from_secs_f64(apply_block_result.commit_time);
        info!(log, "Block was validated with protocol with long processing";
              "commit_time" => format!("{:?}", commit_time_duration),
              "block_header_hash" => block_hash.to_base58_check(),
              "block_header_level" => block.header.level(),
              "context_hash" => apply_block_result.context_hash.to_base58_check(),
              "protocol_call_elapsed" => format!("{:?}", protocol_call_elapsed));
    }

    // Lets mark header as applied and store result
    // store success result
    let store_result_timer = Instant::now();
    let validated_block = ProcessValidatedBlock::new(
        block,
        chain_id,
        apply_block_result.block_metadata_hash.clone(),
        apply_block_result.ops_metadata_hash.clone(),
        Instant::now(),
    );
    let block_additional_data = store_applied_block_result(
        block_storage,
        block_meta_storage,
        &block_hash,
        apply_block_result,
        &mut block_meta,
        cycle_meta_storage,
        cycle_eras_storage,
        constants_storage,
    )?;
    let store_result_elapsed = store_result_timer.elapsed();

    Ok(Some((
        validated_block,
        block_additional_data,
        BlockValidationTimer::new(
            validated_at_timer.elapsed(),
            load_metadata_elapsed,
            protocol_call_elapsed,
            store_result_elapsed,
        ),
    )))
}

/// Collects complete data for applying block, if not complete, return None
#[inline]
fn prepare_apply_request(
    block_hash: &BlockHash,
    chain_id: ChainId,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    operations_storage: &OperationsStorage,
    predecessor_data_cache: Option<(Arc<BlockHeaderWithHash>, BlockAdditionalData)>,
) -> Result<
    (
        ApplyBlockRequest,
        block_meta_storage::Meta,
        Arc<BlockHeaderWithHash>,
    ),
    StorageError,
> {
    // get block header
    let block = match block_storage.get(block_hash)? {
        Some(block) => Arc::new(block),
        None => {
            return Err(StorageError::MissingKey {
                when: format!(
                    "prepare_apply_request (block header not found, block_hash: {}",
                    block_hash.to_base58_check()
                ),
            });
        }
    };

    // get block_metadata
    let block_meta = match block_meta_storage.get(block_hash)? {
        Some(meta) => meta,
        None => {
            return Err(StorageError::MissingKey {
                when: format!(
                    "prepare_apply_request (block header metadata not, block_hash: {}",
                    block_hash.to_base58_check()
                ),
            });
        }
    };

    // get operations
    let operations = operations_storage.get_operations(block_hash)?;

    // resolve predecessor data
    let (
        predecessor,
        (
            predecessor_block_metadata_hash,
            predecessor_ops_metadata_hash,
            predecessor_max_operations_ttl,
        ),
    ) = resolve_block_data(
        block.header.predecessor(),
        block_storage,
        block_meta_storage,
        predecessor_data_cache,
    )
    .map(|(block, additional_data)| (block, additional_data.into()))?;

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

#[inline]
fn resolve_block_data(
    block_hash: &BlockHash,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    block_data_cache: Option<(Arc<BlockHeaderWithHash>, BlockAdditionalData)>,
) -> Result<(Arc<BlockHeaderWithHash>, BlockAdditionalData), StorageError> {
    // check cache at first
    if let Some(cached) = block_data_cache {
        // if cached data are the same as requested, then use it from cache
        if block_hash.eq(&cached.0.hash) {
            return Ok(cached);
        }
    }
    // load data from database
    let block = match block_storage.get(block_hash)? {
        Some(header) => Arc::new(header),
        None => {
            return Err(StorageError::MissingKey {
                when: format!(
                    "resolve_block_data (block header not found, block_hash: {}",
                    block_hash.to_base58_check()
                ),
            });
        }
    };

    // predecessor additional data
    let additional_data = match block_meta_storage.get_additional_data(block_hash)? {
        Some(additional_data) => additional_data,
        None => {
            return Err(StorageError::MissingKey {
                    when: format!("resolve_block_data (block header metadata not found (block was not applied), block_hash: {}", block_hash.to_base58_check()),
                });
        }
    };

    Ok((block, additional_data))
}

/// This initializes ocaml runtime and protocol context,
/// if we start with new databazes without genesis,
/// it ensures correct initialization of storage with genesis and his data.
#[inline]
pub(crate) fn initialize_protocol_context(
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    chain_meta_storage: &ChainMetaStorage,
    operations_meta_storage: &OperationsMetaStorage,
    protocol_controller: &mut ProtocolRunnerConnection,
    tokio_runtime: &tokio::runtime::Handle,
    log: &Logger,
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: &StorageInitInfo,
) -> Result<(), InitializeContextError> {
    let validated_at_timer = Instant::now();

    // we must check if genesis is applied, if not then we need "commit_genesis" to context
    let load_metadata_timer = Instant::now();
    let need_commit_genesis = init_storage_data.replay.is_some()
        || match block_meta_storage.get(&init_storage_data.genesis_block_header_hash)? {
            Some(genesis_meta) => !genesis_meta.is_applied(),
            None => true,
        };

    let load_metadata_elapsed = load_metadata_timer.elapsed();
    trace!(log, "Looking for genesis if applied"; "need_commit_genesis" => need_commit_genesis);

    // initialize protocol context runtime
    let protocol_call_timer = Instant::now();
    let context_init_info = tokio_runtime.block_on(protocol_controller.init_protocol_for_write(
        need_commit_genesis,
        &init_storage_data.patch_context,
        init_storage_data.context_stats_db_path.clone(),
    ))?;

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
                init_storage_data,
                tezos_env,
                &genesis_context_hash,
                log,
            )?;

            // call get additional/json data for genesis (this must be second call, because this triggers context.checkout)
            // this needs to be second step, because, this triggers context.checkout, so we need to call it after store_commit_genesis_result
            let commit_data = tokio_runtime
                .block_on(protocol_controller.genesis_result_data(&genesis_context_hash))?;

            // this, marks genesis block as applied
            let _ = store_commit_genesis_result(
                block_storage,
                block_meta_storage,
                chain_meta_storage,
                operations_meta_storage,
                init_storage_data,
                commit_data,
            )?;
            let store_result_elapsed = store_result_timer.elapsed();

            let mut stats = ApplyBlockStats::default();
            stats.set_applied_block_level(genesis_with_hash.header.level());
            stats.add_block_validation_stats(&BlockValidationTimer::new(
                validated_at_timer.elapsed(),
                load_metadata_elapsed,
                protocol_call_elapsed,
                store_result_elapsed,
            ));

            info!(log, "Genesis commit stored successfully";
                       "stats" => stats.print_formatted_average_times());
        }
    }

    Ok(())
}

const BLOCK_APPLY_DURATION_LONG_TO_LOG: Duration = Duration::from_secs(30);
