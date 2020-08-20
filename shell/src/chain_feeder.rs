// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Sends blocks to the `protocol_runner`.
//! This actor is responsible for correct applying of blocks with Tezos protocol in context
//! This actor is aslo responsible for correct initialization of genesis in storage.

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use failure::{Error, Fail};
use riker::actors::*;
use slog::{debug, info, Logger, trace, warn};

use crypto::hash::{BlockHash, HashType};
use storage::{BlockMetaStorage, BlockStorage, BlockStorageReader, ChainMetaStorage, initialize_storage_with_genesis_block, OperationsMetaStorage, OperationsStorage, OperationsStorageReader, StorageError, StorageInitInfo, store_applied_block_result, store_commit_genesis_result};
use storage::chain_meta_storage::ChainMetaStorageReader;
use storage::persistent::PersistentStorage;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::ApplyBlockRequest;
use tezos_wrapper::service::{IpcCmdServer, ProtocolController, ProtocolServiceError};

use crate::shell_channel::{BlockApplied, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use crate::subscription::subscribe_to_shell_events;

/// This command triggers feeding of completed blocks to the tezos protocol
#[derive(Clone, Debug)]
pub struct FeedChainToProtocol;

type SharedJoinHandle = Arc<Mutex<Option<JoinHandle<Result<(), Error>>>>>;

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(FeedChainToProtocol, ShellChannelMsg)]
pub struct ChainFeeder {
    /// All events from shell will be published to this channel
    shell_channel: ShellChannelRef,
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
        persistent_storage: &PersistentStorage,
        init_storage_data: &StorageInitInfo,
        tezos_env: &TezosEnvironmentConfiguration,
        ipc_server: IpcCmdServer,
        log: Logger) -> Result<ChainFeederRef, CreateError> {
        let apply_block_run = Arc::new(AtomicBool::new(true));
        let block_applier_thread = {
            let apply_block_run = apply_block_run.clone();
            let shell_channel = shell_channel.clone();
            let persistent_storage = persistent_storage.clone();
            let init_storage_data = init_storage_data.clone();
            let tezos_env = tezos_env.clone();

            thread::spawn(move || -> Result<(), Error>{
                let block_storage = BlockStorage::new(&persistent_storage);
                let block_meta_storage = BlockMetaStorage::new(&persistent_storage);
                let chain_meta_storage = ChainMetaStorage::new(&persistent_storage);
                let operations_storage = OperationsStorage::new(&persistent_storage);
                let operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);
                let mut ipc_server = ipc_server;

                while apply_block_run.load(Ordering::Acquire) {
                    match ipc_server.accept() {
                        Ok(protocol_controller) =>
                            match feed_chain_to_protocol(
                                &tezos_env,
                                &init_storage_data,
                                &apply_block_run,
                                &shell_channel,
                                &block_storage,
                                &block_meta_storage,
                                &chain_meta_storage,
                                &operations_storage,
                                &operations_meta_storage,
                                protocol_controller,
                                &log,
                            ) {
                                Ok(()) => debug!(log, "Feed chain to protocol finished"),
                                Err(err) => {
                                    if apply_block_run.load(Ordering::Acquire) {
                                        warn!(log, "Error while feeding chain to protocol"; "reason" => format!("{:?}", err));
                                    }
                                }
                            }
                        Err(err) => warn!(log, "No connection from protocol runner"; "reason" => format!("{:?}", err)),
                    }
                }

                Ok(())
            })
        };

        let myself = sys.actor_of_props::<ChainFeeder>(
            ChainFeeder::name(),
            Props::new_args((shell_channel, apply_block_run, Arc::new(Mutex::new(Some(block_applier_thread))))),
        )?;

        Ok(myself)
    }

    /// The `ChainFeeder` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "chain-feeder"
    }

    fn process_shell_channel_message(&mut self, _ctx: &Context<ChainFeederMsg>, msg: ShellChannelMsg) -> Result<(), Error> {
        match msg {
            ShellChannelMsg::ShuttingDown(_) => {
                self.block_applier_run.store(false, Ordering::Release);
            }
            _ => ()
        }

        Ok(())
    }
}

impl ActorFactoryArgs<(ShellChannelRef, Arc<AtomicBool>, SharedJoinHandle)> for ChainFeeder {
    fn create_args((shell_channel, block_applier_run, block_applier_thread): (ShellChannelRef, Arc<AtomicBool>, SharedJoinHandle)) -> Self {
        ChainFeeder {
            shell_channel,
            block_applier_run,
            block_applier_thread,
        }
    }
}

impl Actor for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_events(&self.shell_channel, ctx.myself());

        ctx.schedule::<Self::Msg, _>(
            Duration::from_secs(4),
            Duration::from_secs(8),
            ctx.myself(),
            None,
            FeedChainToProtocol.into());
    }

    fn post_stop(&mut self) {
        // Set the flag, and let the thread wake up. There is no race condition here, if `unpark`
        // happens first, `park` will return immediately. Hence there is no risk of a deadlock.
        self.block_applier_run.store(false, Ordering::Release);

        let join_handle = self.block_applier_thread.lock().unwrap()
            .take().expect("Thread join handle is missing");
        join_handle.thread().unpark();
        let _ = join_handle.join().expect("Failed to join block applier thread");
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ShellChannelMsg> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match self.process_shell_channel_message(ctx, msg) {
            Ok(_) => (),
            Err(e) => warn!(ctx.system.log(), "Failed to process shell channel message"; "reason" => format!("{:?}", e)),
        }
    }
}

impl Receive<FeedChainToProtocol> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: FeedChainToProtocol, _sender: Sender) {
        if let Some(join_handle) = self.block_applier_thread.lock().unwrap().as_ref() {
            join_handle.thread().unpark();
        }
    }
}

/// Possible errors for feeding chain
#[derive(Debug, Fail)]
pub enum FeedChainError {
    #[fail(display = "Cannot resolve current head, no genesis was commited")]
    UnknownCurrentHeadError,
    #[fail(display = "Storage read/write error! Reason: {:?}", error)]
    StorageError {
        error: StorageError
    },
    #[fail(display = "Protocol service error error! Reason: {:?}", error)]
    ProtocolServiceError {
        error: ProtocolServiceError
    },
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

fn feed_chain_to_protocol(
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: &StorageInitInfo,
    apply_block_run: &AtomicBool,
    shell_channel: &ShellChannelRef,
    block_storage: &BlockStorage,
    block_meta_storage: &BlockMetaStorage,
    chain_meta_storage: &ChainMetaStorage,
    operations_storage: &OperationsStorage,
    operations_meta_storage: &OperationsMetaStorage,
    protocol_controller: ProtocolController,
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
        &protocol_controller,
        &log,
        &tezos_env,
        &init_storage_data,
    )?;

    let chain_id = &init_storage_data.chain_id;

    // now resolve where to start apply next blocks (at least genesis should be there)
    let mut current_head_hash: BlockHash = match &chain_meta_storage.get_current_head(chain_id)? {
        Some(block) => block.hash.clone(),
        None => {
            // this should not happen here, we applied at least genesis before
            return Err(FeedChainError::UnknownCurrentHeadError);
        }
    };

    // now we can start applying block
    while apply_block_run.load(Ordering::Acquire) {
        match block_meta_storage.get(&current_head_hash)? {
            Some(mut current_head_meta) => {
                if current_head_meta.is_applied() {
                    // Current head is already applied, so we should move to successor
                    // or in case no successor is available do nothing.
                    match current_head_meta.successor() {
                        Some(successor_hash) => {
                            current_head_hash = successor_hash.clone();
                            continue;
                        }
                        None => ( /* successor is not yet available, we do nothing for now */ )
                    }
                } else {
                    // Current head is not applied, so we should apply it now.
                    // But first let's fetch current head data from block storage..
                    match block_storage.get(&current_head_hash)? {
                        Some(current_head) => {
                            // Good, we have block data available, let's' look is we have all operations
                            // available. If yes we will apply them. If not, we will do nothing.
                            if operations_meta_storage.is_complete(&current_head.hash)? {
                                debug!(log, "Applying block"; "block_header_hash" => block_hash_encoding.bytes_to_string(&current_head.hash));
                                let operations = operations_storage.get_operations(&current_head_hash)?;

                                // apply block and it's operations
                                let (predecessor, predecessor_additional_data) = match block_storage.get_with_additional_data(&current_head.header.predecessor())? {
                                    None => {
                                        warn!(log, "No data was found in database for the predecessor, so we try to apply it first"; "predecessor_block_header_hash" => block_hash_encoding.bytes_to_string(&current_head.header.predecessor()));
                                        current_head_hash = current_head.header.predecessor().clone();
                                        continue;
                                    }
                                    Some(predecesor_data) => predecesor_data
                                };

                                let apply_block_result = protocol_controller.apply_block(
                                    ApplyBlockRequest {
                                        chain_id: chain_id.clone(),
                                        block_header: (&*current_head.header).clone(),
                                        pred_header: (&*predecessor.header).clone(),
                                        operations: ApplyBlockRequest::convert_operations(operations),
                                        max_operations_ttl: predecessor_additional_data.max_operations_ttl() as i32,
                                    }
                                )?;
                                debug!(
                                    log,
                                    "Block was applied";
                                    "block_header_hash" => block_hash_encoding.bytes_to_string(&current_head.hash),
                                    "context_hash" => HashType::ContextHash.bytes_to_string(&apply_block_result.context_hash),
                                    "validation_result_message" => &apply_block_result.validation_result_message
                                );


                                // store result
                                let (block_json_data, _) = store_applied_block_result(
                                    block_storage,
                                    block_meta_storage,
                                    &current_head.hash,
                                    apply_block_result,
                                    &mut current_head_meta,
                                )?;

                                // notify listeners
                                if apply_block_run.load(Ordering::Acquire) {
                                    // notify others that the block successfully applied
                                    shell_channel.tell(
                                        Publish {
                                            msg: BlockApplied::new(current_head, block_json_data).into(),
                                            topic: ShellChannelTopic::ShellEvents.into(),
                                        }, None);
                                }

                                // Current head is already applied, so we should move to successor
                                // or in case no successor is available do nothing.
                                match current_head_meta.successor() {
                                    Some(successor_hash) => {
                                        current_head_hash = successor_hash.clone();
                                        continue;
                                    }
                                    None => ( /* successor is not yet available, we do nothing for now */ )
                                }
                            } else {
                                // we don't have all operations available, do nothing
                            }
                        }
                        None => ( /* it's possible that data was not yet written do the storage, so don't panic! */ )
                    }
                }
            }
            None => warn!(log, "No meta info record was found in database for the current head"; "block_header_hash" => block_hash_encoding.bytes_to_string(&current_head_hash))
        }

        // This should be hit only in case that the current branch is applied
        // and no successor was available to continue the apply cycle. In that case
        // this thread will be stopped and will wait until it's waked again.
        thread::park();
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
    protocol_controller: &ProtocolController,
    log: &Logger,
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: &StorageInitInfo) -> Result<(), FeedChainError> {

    // we must check if genesis is applied, if not then we need "commit_genesis" to context
    let need_commit_genesis = match block_meta_storage.get(&init_storage_data.genesis_block_header_hash)? {
        Some(genesis_meta) => !genesis_meta.is_applied(),
        None => true,
    };
    trace!(log, "Looking for genesis if applied"; "need_commit_genesis" => need_commit_genesis);

    // initialize protocol context runtime
    let context_init_info = protocol_controller.init_protocol_for_write(need_commit_genesis, &init_storage_data.patch_context)?;
    info!(log, "Protocol context initialized"; "context_init_info" => format!("{:?}", &context_init_info));

    if need_commit_genesis {

        // if we needed commit_genesis, it means, that it is apply of 0 block,
        // which initiates genesis protocol in context, so we need to store some data, like we do in normal apply, see below store_apply_block_result
        if let Some(genesis_context_hash) = context_init_info.genesis_commit_hash {

            // at first store genesis to storage
            let genesis_with_hash = initialize_storage_with_genesis_block(
                block_storage,
                &init_storage_data,
                &tezos_env,
                &genesis_context_hash,
                &log,
            )?;

            // call get additional/json data for genesis (this must be second call, because this triggers context.checkout)
            // this needs to be second step, because, this triggers context.checkout, so we need to call it after store_commit_genesis_result
            let commit_data = protocol_controller.genesis_result_data(&genesis_context_hash)?;

            // this, marks genesis block as applied
            let block_json_data = store_commit_genesis_result(
                block_storage,
                block_meta_storage,
                chain_meta_storage,
                operations_meta_storage,
                &init_storage_data,
                commit_data,
            )?;

            // notify listeners
            if apply_block_run.load(Ordering::Acquire) {
                // notify others that the block successfully applied
                shell_channel.tell(
                    Publish {
                        msg: BlockApplied::new(genesis_with_hash, block_json_data).into(),
                        topic: ShellChannelTopic::ShellEvents.into(),
                    }, None);
            }
        }
    }

    Ok(())
}