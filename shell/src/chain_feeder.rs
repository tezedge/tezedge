// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use failure::Error;
use riker::actors::*;
use slog::{debug, Logger, warn};

use crypto::hash::{BlockHash, ChainId, HashType};
use storage::{BlockJsonDataBuilder, BlockMetaStorage, BlockStorage, BlockStorageReader, OperationsMetaStorage, OperationsStorage, OperationsStorageReader};
use storage::persistent::PersistentStorage;
use tezos_api::client::TezosStorageInitInfo;
use tezos_wrapper::service::{IpcCmdServer, ProtocolController};

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

pub type ChainFeederRef = ActorRef<ChainFeederMsg>;

impl ChainFeeder {
    pub fn actor(sys: &impl ActorRefFactory, shell_channel: ShellChannelRef, persistent_storage: &PersistentStorage, tezos_init: &TezosStorageInitInfo, ipc_server: IpcCmdServer, log: Logger) -> Result<ChainFeederRef, CreateError> {
        let apply_block_run = Arc::new(AtomicBool::new(true));
        let block_applier_thread = {
            let apply_block_run = apply_block_run.clone();
            let current_head_hash = tezos_init.current_block_header_hash.clone();
            let chain_id = tezos_init.chain_id.clone();
            let shell_channel = shell_channel.clone();
            let persistent_storage = persistent_storage.clone();

            thread::spawn(move || {
                let mut block_storage = BlockStorage::new(&persistent_storage);
                let mut block_meta_storage = BlockMetaStorage::new(&persistent_storage);
                let operations_storage = OperationsStorage::new(&persistent_storage);
                let operations_meta_storage = OperationsMetaStorage::new(&persistent_storage);
                let mut ipc_server = ipc_server;

                while apply_block_run.load(Ordering::Acquire) {
                    match ipc_server.accept() {
                        Ok(protocol_controller) =>
                            match feed_chain_to_protocol(&chain_id, &apply_block_run, &current_head_hash, &shell_channel, &mut block_storage, &mut block_meta_storage, &operations_storage, &operations_meta_storage, protocol_controller, &log) {
                                Ok(()) => debug!(log, "Feed chain to protocol finished"),
                                Err(err) => {
                                    if apply_block_run.load(Ordering::Acquire) {
                                        warn!(log, "Error while feeding chain to protocol"; "reason" => format!("{:?}", err));
                                    }
                                },
                            }
                        Err(err) => warn!(log, "No connection from protocol runner"; "reason" => format!("{:?}", err)),
                    }
                }

                Ok(())
            })
        };

        let myself = sys.actor_of(
            Props::new_args(ChainFeeder::new, (shell_channel, apply_block_run, Arc::new(Mutex::new(Some(block_applier_thread))))),
            ChainFeeder::name())?;

        Ok(myself)
    }

    /// The `ChainFeeder` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "chain-feeder"
    }

    fn new((shell_channel, block_applier_run, block_applier_thread): (ShellChannelRef, Arc<AtomicBool>, SharedJoinHandle)) -> Self {
        ChainFeeder {
            shell_channel,
            block_applier_run,
            block_applier_thread,
        }
    }

    fn process_shell_channel_message(&mut self, _ctx: &Context<ChainFeederMsg>, msg: ShellChannelMsg) -> Result<(), Error> {
        match msg {
            ShellChannelMsg::ShuttingDown(_) => {
                self.block_applier_run.store(false, Ordering::Release);
            },
            _ => ()
        }

        Ok(())
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


fn feed_chain_to_protocol(
    chain_id: &ChainId,
    apply_block_run: &AtomicBool,
    current_head_hash: &BlockHash,
    shell_channel: &ShellChannelRef,
    block_storage: &mut BlockStorage,
    block_meta_storage: &mut BlockMetaStorage,
    operations_storage: &OperationsStorage,
    operations_meta_storage: &OperationsMetaStorage,
    protocol_controller: ProtocolController,
    log: &Logger,
) -> Result<(), Error> {
    let block_hash_encoding = HashType::BlockHash;
    let mut current_head_hash = current_head_hash.clone();

    protocol_controller.init_protocol()?;

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
                                let operations = operations_storage.get_operations(&current_head_hash)?
                                    .drain(..)
                                    .map(Some)
                                    .collect();
                                // apply block and it's operations
                                let apply_block_result = protocol_controller.apply_block(&chain_id, &current_head.header, &operations)?;
                                debug!(log, "Block was applied";"block_header_hash" => block_hash_encoding.bytes_to_string(&current_head.hash), "validation_result_message" => apply_block_result.validation_result_message);
                                // mark current head as applied
                                current_head_meta.set_is_applied(true);
                                block_meta_storage.put(&current_head.hash, &current_head_meta)?;
                                // store json data
                                let block_json_data = BlockJsonDataBuilder::default()
                                    .block_header_proto_json(apply_block_result.block_header_proto_json)
                                    .block_header_proto_metadata_json(apply_block_result.block_header_proto_metadata_json)
                                    .operations_proto_metadata_json(apply_block_result.operations_proto_metadata_json)
                                    .build().unwrap();
                                block_storage.put_block_json_data(&current_head.hash, block_json_data.clone())?;
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

