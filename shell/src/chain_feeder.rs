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
use slog::{debug, error, info, trace, warn, Logger};

use crypto::hash::{BlockHash, ChainId, ContextHash, HashType};
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::context::{ContextApi, TezedgeContext};
use storage::persistent::PersistentStorage;
use storage::{
    initialize_storage_with_genesis_block, store_applied_block_result, store_commit_genesis_result,
    BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader, ChainMetaStorage,
    OperationsMetaStorage, OperationsStorage, OperationsStorageReader, StorageError,
    StorageInitInfo,
};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::ApplyBlockRequest;
use tezos_wrapper::service::{
    handle_protocol_service_error, ProtocolController, ProtocolServiceError,
};
use tezos_wrapper::TezosApiConnectionPool;

use crate::chain_manager::{ChainManagerRef, ProcessValidatedBlock};
use crate::shell_channel::{ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use crate::stats::BlockValidationTimer;
use crate::subscription::subscribe_to_shell_shutdown;
use crate::utils::{dispatch_condvar_result, CondvarResult};
use crate::validation;

type SharedJoinHandle = Arc<Mutex<Option<JoinHandle<Result<(), Error>>>>>;

/// Message commands [`ChainFeeder`] to apply completed block.
#[derive(Clone, Debug)]
pub struct ApplyCompletedBlock {
    block_hash: BlockHash,
    chain_id: Arc<ChainId>,
    roundtrip_timer: Arc<Instant>,
    chain_manager: Arc<ChainManagerRef>,
    /// Callback can be used to wait for apply block result
    result_callback: Option<CondvarResult<(), failure::Error>>,
}

impl ApplyCompletedBlock {
    pub fn new(
        block_hash: BlockHash,
        chain_id: Arc<ChainId>,
        result_callback: Option<CondvarResult<(), failure::Error>>,
        chain_manager: Arc<ChainManagerRef>,
        roundtrip_timer: Instant,
    ) -> Self {
        Self {
            block_hash,
            chain_id,
            result_callback,
            chain_manager,
            roundtrip_timer: Arc::new(roundtrip_timer),
        }
    }
}

/// Message commands [`ChainFeeder`] to check blocks, if they could be applied.
#[derive(Clone, Debug)]
pub struct CheckBlocksForApply {
    blocks: Vec<BlockHash>,
    chain_id: Arc<ChainId>,
    chain_manager: Arc<ChainManagerRef>,
    roundtrip_timer: Instant,
}

impl CheckBlocksForApply {
    pub fn new(
        blocks: Vec<BlockHash>,
        chain_id: Arc<ChainId>,
        chain_manager: Arc<ChainManagerRef>,
        roundtrip_timer: Instant,
    ) -> Self {
        Self {
            chain_id,
            blocks,
            chain_manager,
            roundtrip_timer,
        }
    }
}

/// Message commands [`ChainFeeder`] to apply block.
pub struct ApplyBlock {
    envelope: ApplyCompletedBlock,
    chain_feeder: ChainFeederRef,
    request: ApplyBlockRequest,
}

impl ApplyBlock {
    pub(crate) fn new(
        envelope: ApplyCompletedBlock,
        chain_feeder: ChainFeederRef,
        request: ApplyBlockRequest,
    ) -> Self {
        ApplyBlock {
            envelope,
            chain_feeder,
            request,
        }
    }
}

/// Internal queue commands
pub(crate) enum Event {
    ApplyBlock(ApplyBlock),
    ShuttingDown,
}

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(ShellChannelMsg, ApplyCompletedBlock, CheckBlocksForApply)]
pub struct ChainFeeder {
    /// Just for subscribing to shell shutdown channel
    shell_channel: ShellChannelRef,

    /// Block storage
    block_storage: Box<dyn BlockStorageReader>,
    /// Block meta storage
    block_meta_storage: Box<dyn BlockMetaStorageReader>,
    /// Operations storage
    operations_storage: Box<dyn OperationsStorageReader>,
    /// Operations meta storage
    operations_meta_storage: OperationsMetaStorage,

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
                shell_channel.clone(),
                persistent_storage.clone(),
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
                persistent_storage,
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

    /// This should be called by [ApplyCompletedBlock], only if we have block which can be applied [chain_state.can_apply_block]
    fn apply_completed_block(
        &self,
        msg: ApplyCompletedBlock,
        chain_feeder: ChainFeederRef,
        log: &Logger,
    ) -> Result<(), Error> {
        // check if block is already applied (not necessray here)
        match self.block_meta_storage.get(&msg.block_hash)? {
            Some(meta) => {
                if meta.is_applied() {
                    // block already applied - ok, doing nothing
                    debug!(log, "Block is already applied"; "block" => HashType::BlockHash.hash_to_b58check(&msg.block_hash));
                    return Ok(());
                }
            }
            None => {
                return Err(format_err!(
                    "Block metadata not found for block_hash: {}",
                    HashType::BlockHash.hash_to_b58check(&msg.block_hash)
                ));
            }
        }

        // collect data
        let request = self.prepare_apply_request(&msg.block_hash, msg.chain_id.as_ref().clone())?;

        // add request to queue
        let result_callback = msg.result_callback.clone();
        match self.send_to_queue(Event::ApplyBlock(ApplyBlock::new(
            msg,
            chain_feeder,
            request,
        ))) {
            Ok(_) => Ok(()),
            Err(e) => {
                let err = format_err!(
                    "Failed to send `apply block request` to queue, reason: {}",
                    format!("{}", e)
                );
                if let Err(e) = dispatch_condvar_result(result_callback, || Err(e), true) {
                    warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                }
                Err(err)
            }
        }
    }

    /// Collects complete data for applying block, if not complete, return None
    fn prepare_apply_request(
        &self,
        block_hash: &BlockHash,
        chain_id: ChainId,
    ) -> Result<ApplyBlockRequest, StorageError> {
        // get block header
        let current_head = match self.block_storage.get(block_hash)? {
            Some(block) => block,
            None => return Err(StorageError::MissingKey),
        };

        // get operations
        let operations = self.operations_storage.get_operations(block_hash)?;

        // get predecessor metadata
        let (
            predecessor,
            (
                predecessor_block_metadata_hash,
                predecessor_ops_metadata_hash,
                predecessor_max_operations_ttl,
            ),
        ) = match self
            .block_storage
            .get_with_additional_data(&current_head.header.predecessor())?
        {
            Some((predecessor, predecessor_additional_data)) => {
                (predecessor, predecessor_additional_data.into())
            }
            None => return Err(StorageError::MissingKey),
        };

        Ok(ApplyBlockRequest {
            chain_id,
            block_header: (&*current_head.header).clone(),
            pred_header: (&*predecessor.header).clone(),
            operations: ApplyBlockRequest::convert_operations(operations),
            max_operations_ttl: predecessor_max_operations_ttl as i32,
            predecessor_block_metadata_hash,
            predecessor_ops_metadata_hash,
        })
    }

    fn check_blocks_for_apply(
        &self,
        msg: CheckBlocksForApply,
        chain_feeder: ChainFeederRef,
        log: &Logger,
    ) -> Result<(), Error> {
        for block in &msg.blocks {
            if let Some(block_metadata) = self.block_meta_storage.get(&block)? {
                if validation::can_apply_block(
                    (&block, &block_metadata),
                    |bh| self.operations_meta_storage.is_complete(bh),
                    |predecessor| self.block_meta_storage.is_applied(predecessor),
                )? {
                    self.apply_completed_block(
                        ApplyCompletedBlock::new(
                            block.clone(),
                            msg.chain_id.clone(),
                            None,
                            msg.chain_manager.clone(),
                            Instant::now(),
                        ),
                        chain_feeder.clone(),
                        log,
                    )?;
                }
            }
        }

        Ok(())
    }
}

impl
    ActorFactoryArgs<(
        ShellChannelRef,
        PersistentStorage,
        Arc<Mutex<QueueSender<Event>>>,
        Arc<AtomicBool>,
        SharedJoinHandle,
    )> for ChainFeeder
{
    fn create_args(
        (
            shell_channel,
            persistent_storage,
            block_applier_event_sender,
            block_applier_run,
            block_applier_thread,
        ): (
            ShellChannelRef,
            PersistentStorage,
            Arc<Mutex<QueueSender<Event>>>,
            Arc<AtomicBool>,
            SharedJoinHandle,
        ),
    ) -> Self {
        ChainFeeder {
            shell_channel,
            block_storage: Box::new(BlockStorage::new(&persistent_storage)),
            block_meta_storage: Box::new(BlockMetaStorage::new(&persistent_storage)),
            operations_storage: Box::new(OperationsStorage::new(&persistent_storage)),
            operations_meta_storage: OperationsMetaStorage::new(&persistent_storage),
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

impl Receive<ApplyCompletedBlock> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ApplyCompletedBlock, _sender: Sender) {
        if !self.block_applier_run.load(Ordering::Acquire) {
            return;
        }

        match self.apply_completed_block(msg, ctx.myself().clone(), &ctx.system.log()) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Failed to apply completed block"; "reason" => format!("{:?}", e))
            }
        }
    }
}

impl Receive<CheckBlocksForApply> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: CheckBlocksForApply, _sender: Sender) {
        if !self.block_applier_run.load(Ordering::Acquire) {
            return;
        }

        match self.check_blocks_for_apply(msg, ctx.myself().clone(), &ctx.system.log()) {
            Ok(_) => (),
            Err(e) => {
                warn!(ctx.system.log(), "Failed to check blocks for apply"; "reason" => format!("{:?}", e))
            }
        }
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
    #[fail(display = "Context is not stored, context_hash: {}", context_hash)]
    MissingContextError { context_hash: String },
    #[fail(display = "Storage read/write error! Reason: {:?}", error)]
    StorageError { error: StorageError },
    #[fail(display = "Protocol service error error! Reason: {:?}", error)]
    ProtocolServiceError { error: ProtocolServiceError },
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
    shell_channel: ShellChannelRef,
    persistent_storage: PersistentStorage,
    init_storage_data: Arc<StorageInitInfo>,
    tezos_env: Arc<TezosEnvironmentConfiguration>,
    tezos_writeable_api: Arc<TezosApiConnectionPool>,
    log: Logger,
}

impl BlockApplierThreadSpawner {
    pub(crate) fn new(
        shell_channel: ShellChannelRef,
        persistent_storage: PersistentStorage,
        init_storage_data: Arc<StorageInitInfo>,
        tezos_env: Arc<TezosEnvironmentConfiguration>,
        tezos_writeable_api: Arc<TezosApiConnectionPool>,
        log: Logger,
    ) -> Self {
        Self {
            shell_channel,
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
            let shell_channel = self.shell_channel.clone();
            let persistent_storage = self.persistent_storage.clone();
            let tezos_writeable_api = self.tezos_writeable_api.clone();
            let init_storage_data = self.init_storage_data.clone();
            let tezos_env = self.tezos_env.clone();
            let log = self.log.clone();
            let block_applier_run = block_applier_run.clone();

            // TOOD: meno
            thread::spawn(move || -> Result<(), Error> {
                let block_storage = BlockStorage::new(&persistent_storage);
                let block_meta_storage = BlockMetaStorage::new(&persistent_storage);
                let chain_meta_storage = ChainMetaStorage::new(&persistent_storage);
                let operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);
                let context: Box<dyn ContextApi> = Box::new(TezedgeContext::new(
                    block_storage.clone(),
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
                            &shell_channel,
                            &block_storage,
                            &block_meta_storage,
                            &chain_meta_storage,
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
    shell_channel: &ShellChannelRef,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    chain_meta_storage: &ChainMetaStorage,
    operations_meta_storage: &OperationsMetaStorage,
    context: &Box<dyn ContextApi>,
    protocol_controller: &ProtocolController,
    block_applier_event_receiver: &mut QueueReceiver<Event>,
    log: &Logger,
) -> Result<(), FeedChainError> {
    let block_hash_encoding = HashType::BlockHash;

    // at first we initialize protocol runtime and ffi context
    initialize_protocol_context(
        &apply_block_run,
        &shell_channel,
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
                Event::ApplyBlock(ApplyBlock {
                    envelope:
                        ApplyCompletedBlock {
                            block_hash,
                            result_callback,
                            chain_manager,
                            roundtrip_timer,
                            chain_id,
                        },
                    chain_feeder,
                    request,
                }) => {
                    let validated_at_timer = Instant::now();
                    debug!(log, "Applying block"; "block_header_hash" => block_hash_encoding.hash_to_b58check(&block_hash));

                    // check if block is already applied (not necessery here)
                    let load_metadata_timer = Instant::now();
                    let mut current_head_meta = match block_meta_storage.get(&block_hash)? {
                        Some(meta) => {
                            if meta.is_applied() {
                                // block already applied - ok, doing nothing
                                debug!(log, "Block is already applied (feeder)"; "block" => HashType::BlockHash.hash_to_b58check(&block_hash));
                                if let Err(e) = dispatch_condvar_result(
                                    result_callback,
                                    || Err(format_err!("Block is already applied")),
                                    true,
                                ) {
                                    warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                                }
                                continue;
                            }
                            meta
                        }
                        None => {
                            warn!(log, "Block metadata not found (feeder)"; "block" => HashType::BlockHash.hash_to_b58check(&block_hash));
                            if let Err(e) = dispatch_condvar_result(
                                result_callback,
                                || Err(format_err!("Block metadata not found")),
                                true,
                            ) {
                                warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                            }
                            continue;
                        }
                    };
                    let load_metadata_elapsed = load_metadata_timer.elapsed();

                    // try apply block
                    let protocol_call_timer = Instant::now();
                    match protocol_controller.apply_block(request) {
                        Ok(apply_block_result) => {
                            let protocol_call_elapsed = protocol_call_timer.elapsed();
                            debug!(log, "Block was applied";
                                "block_header_hash" => block_hash_encoding.hash_to_b58check(&block_hash),
                                "context_hash" => HashType::ContextHash.hash_to_b58check(&apply_block_result.context_hash),
                                "validation_result_message" => &apply_block_result.validation_result_message);

                            // we need to check and wait for context_hash to be 100% sure, that everything is ok
                            let context_wait_timer = Instant::now();
                            if let Err(e) =
                                wait_for_context(context, &apply_block_result.context_hash)
                            {
                                error!(log,
                                      "Failed to wait for context";
                                      "block" => HashType::BlockHash.hash_to_b58check(&block_hash),
                                      "context" => HashType::ContextHash.hash_to_b58check(&apply_block_result.context_hash),
                                      "reason" => format!("{}", e)
                                );
                                if let Err(e) = dispatch_condvar_result(
                                    result_callback,
                                    || {
                                        Err(format_err!("Failed to wait for context, context_hash: {}, reason: {}", HashType::ContextHash.hash_to_b58check(&apply_block_result.context_hash), e)
                                        )
                                    },
                                    true,
                                ) {
                                    warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                                }
                                return Err(FeedChainError::MissingContextError {
                                    context_hash: HashType::ContextHash
                                        .hash_to_b58check(&apply_block_result.context_hash),
                                });
                            }
                            let context_wait_elapsed = context_wait_timer.elapsed();
                            if context_wait_elapsed.gt(&CONTEXT_WAIT_DURATION_LONG_TO_LOG) {
                                info!(log, "Block was applied with long context processing";
                                           "block_header_hash" => block_hash_encoding.hash_to_b58check(&block_hash),
                                           "context_hash" => HashType::ContextHash.hash_to_b58check(&apply_block_result.context_hash),
                                           "context_wait_elapsed" => format!("{:?}", &context_wait_elapsed));
                            }

                            // Lets mark header as applied and store result
                            // store success result
                            let store_result_timer = Instant::now();
                            match store_applied_block_result(
                                block_storage,
                                block_meta_storage,
                                &block_hash,
                                apply_block_result,
                                &mut current_head_meta,
                            ) {
                                Ok(_) => {
                                    // now everythings stored, we are done

                                    // notify condvar
                                    if let Err(e) =
                                        dispatch_condvar_result(result_callback, || Ok(()), true)
                                    {
                                        warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                                    }
                                }
                                Err(e) => {
                                    if let Err(e) = dispatch_condvar_result(
                                        result_callback,
                                        || {
                                            Err(format_err!(
                                                "Failed to store applied result, reason: {}",
                                                e
                                            ))
                                        },
                                        true,
                                    ) {
                                        warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                                    }
                                    return Err(e.into());
                                }
                            };
                            let store_result_elapsed = store_result_timer.elapsed();

                            // notify chain_manager, which sent request
                            if apply_block_run.load(Ordering::Acquire) {
                                // now we want to parallelize and speed-up

                                // 1. ping chain_feeder for successors check -> to queue
                                let successors = current_head_meta.take_successors();
                                if !successors.is_empty() {
                                    chain_feeder.tell(
                                        CheckBlocksForApply::new(
                                            successors,
                                            chain_id,
                                            chain_manager.clone(),
                                            Instant::now(),
                                        ),
                                        None,
                                    );
                                }

                                // 2. ping chain_managers
                                chain_manager.tell(
                                    ProcessValidatedBlock::new(
                                        Arc::new(block_hash),
                                        roundtrip_timer,
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
                        Err(pse) => {
                            if let Err(e) = dispatch_condvar_result(
                                result_callback,
                                || Err(format_err!("{}", pse)),
                                true,
                            ) {
                                warn!(log, "Failed to dispatch result to condvar"; "reason" => format!("{}", e));
                            }
                            handle_protocol_service_error(
                                pse,
                                |e| warn!(log, "Failed to apply block"; "block" => HashType::BlockHash.hash_to_b58check(&block_hash), "reason" => format!("{:?}", e)),
                            )?;
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

/// This initializes ocaml runtime and protocol context,
/// if we start with new databazes without genesis,
/// it ensures correct initialization of storage with genesis and his data.
pub(crate) fn initialize_protocol_context(
    apply_block_run: &AtomicBool,
    shell_channel: &ShellChannelRef,
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
    let roundtrip_timer = Instant::now();

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
    info!(log, "Protocol context initialized"; "context_init_info" => format!("{:?}", &context_init_info));

    if need_commit_genesis {
        // if we needed commit_genesis, it means, that it is apply of 0 block,
        // which initiates genesis protocol in context, so we need to store some data, like we do in normal apply, see below store_apply_block_result
        if let Some(genesis_context_hash) = context_init_info.genesis_commit_hash {
            // at first store genesis to storage
            let store_result_timer = Instant::now();
            let genesis_with_hash = initialize_storage_with_genesis_block(
                block_storage,
                &init_storage_data,
                &tezos_env,
                &genesis_context_hash,
                &log,
            )?;

            let context_wait_timer = Instant::now();
            if let Err(e) = wait_for_context(context, &genesis_context_hash) {
                error!(log,
                       "Failed to wait for genesis context";
                       "block" => HashType::BlockHash.hash_to_b58check(&init_storage_data.genesis_block_header_hash),
                       "context" => HashType::ContextHash.hash_to_b58check(&genesis_context_hash),
                       "reason" => format!("{}", e)
                );
                return Err(FeedChainError::MissingContextError {
                    context_hash: HashType::ContextHash.hash_to_b58check(&genesis_context_hash),
                });
            }
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
                shell_channel.tell(
                    Publish {
                        msg: ShellChannelMsg::ProcessValidatedGenesisBlock(
                            ProcessValidatedBlock::new(
                                Arc::new(genesis_with_hash.hash),
                                Arc::new(roundtrip_timer),
                                Arc::new(BlockValidationTimer::new(
                                    validated_at_timer.elapsed(),
                                    load_metadata_elapsed,
                                    protocol_call_elapsed,
                                    context_wait_elapsed,
                                    store_result_elapsed,
                                )),
                            ),
                        ),
                        topic: ShellChannelTopic::ShellCommands.into(),
                    },
                    None,
                );
            }
        }
    }

    Ok(())
}

const CONTEXT_WAIT_DURATION: (Duration, Duration) =
    (Duration::from_secs(300), Duration::from_millis(10));
const CONTEXT_WAIT_DURATION_LONG_TO_LOG: Duration = Duration::from_secs(30);

/// Context_listener is now asynchronous, so we need to make sure, that it is processed, so we wait a little bit
pub fn wait_for_context(
    context: &Box<dyn ContextApi>,
    context_hash: &ContextHash,
) -> Result<(), failure::Error> {
    let (timeout, delay): (Duration, Duration) = CONTEXT_WAIT_DURATION;
    let start = SystemTime::now();

    // try find context_hash
    loop {
        // if success, than ok
        if let Ok(true) = context.is_committed(context_hash) {
            break Ok(());
        }

        // kind of simple retry policy
        if start.elapsed()?.le(&timeout) {
            thread::sleep(delay);
        } else {
            break Err(failure::format_err!("Block inject - context was not processed for context_hash: {}, timeout (timeout: {:?}, delay: {:?})", HashType::ContextHash.hash_to_b58check(&context_hash), timeout, delay));
        }
    }
}
