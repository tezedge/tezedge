// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! TODO:
//! This actor is responsible for ...

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;

use failure::{Error, Fail};
use riker::actors::*;
use slog::{debug, info, Logger, trace, warn};

use crypto::hash::{BlockHash, HashType, OperationHash};
use storage::{BlockMetaStorage, BlockMetaStorageReader, BlockStorage, BlockStorageReader, MempoolStorage, StorageError, StorageInitInfo};
use storage::persistent::PersistentStorage;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::{PrevalidatorWrapper, ValidateOperationResult};
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::mempool::Mempool;
use tezos_messages::p2p::encoding::operation::MempoolOperationType;
use tezos_wrapper::service::{IpcCmdServer, ProtocolController, ProtocolServiceError};

use crate::Head;
use crate::shell_channel::{ShellChannelMsg, ShellChannelRef};
use crate::subscription::subscribe_to_shell_events;

type SharedJoinHandle = Arc<Mutex<Option<JoinHandle<Result<(), Error>>>>>;

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(ShellChannelMsg)]
pub struct MempoolPrevalidator {
    /// All events from shell will be published to this channel
    shell_channel: ShellChannelRef,
    queue: Arc<Mutex<VecDeque<Event>>>,
    validator_run: Arc<AtomicBool>,
    validator_thread: SharedJoinHandle,
}

enum Event {
    NewHead(BlockHash, Level),
    ValidateOperation(OperationHash, MempoolOperationType),
}

/// Reference to [chain feeder](ChainFeeder) actor
pub type MempoolPrevalidatorRef = ActorRef<MempoolPrevalidatorMsg>;

impl MempoolPrevalidator {
    pub fn actor(
        sys: &impl ActorRefFactory,
        shell_channel: ShellChannelRef,
        persistent_storage: &PersistentStorage,
        init_storage_data: &StorageInitInfo,
        tezos_env: &TezosEnvironmentConfiguration,
        (ipc_server, endpoint_name): (IpcCmdServer, String),
        log: Logger) -> Result<MempoolPrevalidatorRef, CreateError> {

        // spawn thread which processes commands from queue
        let queue = Arc::new(Mutex::new(VecDeque::new()));
        let validator_run = Arc::new(AtomicBool::new(true));
        let validator_thread = {
            let persistent_storage = persistent_storage.clone();
            let shell_channel = shell_channel.clone();
            let init_storage_data = init_storage_data.clone();
            let tezos_env = tezos_env.clone();
            let validator_run = validator_run.clone();
            let endpoint_name = endpoint_name.clone();
            let mut ipc_server = ipc_server;
            let queue = queue.clone();

            thread::spawn(move || {
                let mut block_storage = BlockStorage::new(&persistent_storage);
                let mut block_meta_storage = BlockMetaStorage::new(&persistent_storage);
                let mut mempool_storage = MempoolStorage::new(&persistent_storage);

                while validator_run.load(Ordering::Acquire) {
                    match ipc_server.accept() {
                        Ok(protocol_controller) =>
                            match process_prevalidation(&tezos_env, &mut block_storage, &mut block_meta_storage, &mut mempool_storage, &init_storage_data, &validator_run, &shell_channel, protocol_controller, &queue, &log) {
                                Ok(()) => debug!(log, "Mempool - prevalidation process finished"; "endpoint" => endpoint_name.clone()),
                                Err(err) => {
                                    if validator_run.load(Ordering::Acquire) {
                                        warn!(log, "Mempool - error while process prevalidation"; "endpoint" => endpoint_name.clone(), "reason" => format!("{:?}", err));
                                    }
                                }
                            }
                        Err(err) => warn!(log, "Mempool - no connection from protocol runner"; "endpoint" => endpoint_name.clone(), "reason" => format!("{:?}", err)),
                    }
                }
                Ok(())
            })
        };

        // create actor
        let myself = sys.actor_of(
            Props::new_args(MempoolPrevalidator::new, (shell_channel, validator_run, Arc::new(Mutex::new(Some(validator_thread))), queue)),
            MempoolPrevalidator::name(),
        )?;

        Ok(myself)
    }

    /// The `MempoolPrevalidator` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "mempool-prevalidator"
    }

    fn new((shell_channel, validator_run, validator_thread, queue): (ShellChannelRef, Arc<AtomicBool>, SharedJoinHandle, Arc<Mutex<VecDeque<Event>>>)) -> Self {
        MempoolPrevalidator {
            shell_channel,
            validator_run,
            validator_thread,
            queue,
        }
    }

    fn process_shell_channel_message(&mut self, ctx: &Context<MempoolPrevalidatorMsg>, msg: ShellChannelMsg) -> Result<(), Error> {
        match msg {
            ShellChannelMsg::BlockApplied(block) => {
                // add NewHead to queue
                self.queue.lock().unwrap().push_back(Event::NewHead(block.header().hash.clone(), block.header().header.level().clone()));

                if let Some(join_handle) = self.validator_thread.lock().unwrap().as_ref() {
                    join_handle.thread().unpark();
                }
            }
            ShellChannelMsg::MempoolOperationReceived(operation) => {
                // add operation to queue for validation
                self.queue.lock().unwrap().push_back(Event::ValidateOperation(operation.operation_hash.clone(), operation.operation_type));

                if let Some(join_handle) = self.validator_thread.lock().unwrap().as_ref() {
                    join_handle.thread().unpark();
                }
            }
            _ => ()
        }

        Ok(())
    }
}

impl Actor for MempoolPrevalidator {
    type Msg = MempoolPrevalidatorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_events(&self.shell_channel, ctx.myself());
    }

    fn post_stop(&mut self) {
        self.validator_run.store(false, Ordering::Release);
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

#[derive(Clone, Debug)]
pub struct MempoolState {
    prevalidator: Option<PrevalidatorWrapper>,
    predecessor: Option<Head>,
    mempool: Mempool,
    validation_result: ValidateOperationResult,
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
    tezos_env: &TezosEnvironmentConfiguration,
    block_storage: &mut BlockStorage,
    block_meta_storage: &mut BlockMetaStorage,
    mempool_storage: &mut MempoolStorage,
    init_storage_data: &StorageInitInfo,
    validator_run: &AtomicBool,
    shell_channel: &ShellChannelRef,
    protocol_controller: ProtocolController,
    queue: &Mutex<VecDeque<Event>>,
    log: &Logger,
) -> Result<(), PrevalidationError> {
    let result = protocol_controller.init_protocol(false, true)?;
    info!(log, "Mempool - protocol context (readonly) initialized for mempool");

    // hydrate state
    let mut state = hydrate_state(block_storage, block_meta_storage, mempool_storage, &protocol_controller, &init_storage_data, &log)?;

    while validator_run.load(Ordering::Acquire) {
        match queue.lock().unwrap().pop_front() {
            None => trace!(log, "Mempool - no events is queue, so just wait for some"),
            Some(event) => match event {
                Event::NewHead(header, level) => {

                    // check if NewHeader is bigger than actual, if present
                    if let Some(current_mempool_block) = &mut state.predecessor {
                        if level <= current_mempool_block.level {
                            trace!(log, "Mempool - new head has smaller level than actual mempool head, so we just ignore it!";
                                        "received_level" => level,
                                        "received_block_hash" => HashType::BlockHash.bytes_to_string(&header),
                                        "current_mempool_level" => current_mempool_block.level,
                                        "current_mempool_block_hash" => HashType::BlockHash.bytes_to_string(&current_mempool_block.hash));
                            continue;
                        }
                    }

                    debug!(log, "Mempool - new head received, so begin construction a new context";
                                "received_level" => level,
                                "received_block_hash" => HashType::BlockHash.bytes_to_string(&header));

                    // try to begin construction
                    let (prevalidator, head) = begin_construction(block_storage, &protocol_controller, &init_storage_data, &header, &log)?;

                    // reset state
                    state.predecessor = head;
                    state.prevalidator = prevalidator;
                    continue;
                }
                Event::ValidateOperation(oph, mempoolOperationType) => {
                    info!(log, "[Mempool] validate operation"; "hash" => HashType::OperationHash.bytes_to_string(&oph));

                    // TODO: handling when to exists
                    let operation = mempool_storage.get(mempoolOperationType, oph)?;
                    if let Some(operation) = operation {

                        // TODO: handle if empty
                        match &mut state.prevalidator {
                            None => (),
                            Some(p) => {
                                let response = protocol_controller.validate_operation(&p, operation.operation());
                                match response {
                                    Ok(response) => {
                                        let result = response.result;
                                        info!(log, "Mempool - validate operation response success "; "result" => format!("{:?}", result));
                                        // TODO: handle result
                                    }
                                    Err(err) => warn!(log, "[Mempool] Failed to validate operation message"; "error" => format!("{:?}", err)),
                                }
                            }
                        }
                    }
                }
            }
        }

        // This should be hit only in case that there is nothing to process, so we just wait for data
        thread::park();
    }

    Ok(())
}

fn hydrate_state(
    block_storage: &mut BlockStorage,
    block_meta_storage: &mut BlockMetaStorage,
    mempool_storage: &mut MempoolStorage,
    protocol_controller: &ProtocolController,
    init_storage_data: &StorageInitInfo,
    log: &Logger) -> Result<MempoolState, PrevalidationError> {

    // load current head
    let current_head = block_meta_storage.load_current_head()?
        .map(|(hash, level)| Head { hash, level });

    // begin construction for a current head
    let (prevalidator, predecessor) = match current_head {
        Some(head) => begin_construction(block_storage, protocol_controller, init_storage_data, &head.hash, &log)?,
        None => (None, None)
    };

    // internal mempool state
    Ok(
        MempoolState {
            prevalidator,
            predecessor,
            mempool: Mempool::default(),
            validation_result: ValidateOperationResult::default(),
        }
    )
}

fn begin_construction(block_storage: &mut BlockStorage,
                      protocol_controller: &ProtocolController,
                      init_storage_data: &StorageInitInfo,
                      block_hash: &BlockHash,
                      log: &Logger) -> Result<(Option<PrevalidatorWrapper>, Option<Head>), PrevalidationError> {
    // read whole header
    let result = block_storage.get(&block_hash)?
        .map_or((None, None), |block| {
            // try to begin construction
            match protocol_controller.begin_construction(&init_storage_data.chain_id, &block.header) {
                Ok(prevalidator) => (
                    Some(prevalidator),
                    Some(
                        Head {
                            hash: block.hash,
                            level: block.header.level(),
                        }
                    )
                ),
                Err(err) => {
                    warn!(log, "Mempool - failed to begin construction"; "block_hash" => HashType::BlockHash.bytes_to_string(&block_hash), "error" => format!("{:?}", err));
                    (None, None)
                }
            }
        });
    Ok(result)
}

