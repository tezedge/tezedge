// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::thread::JoinHandle;

use failure::Error;
use riker::actors::*;
use slog::{debug, Logger, warn};

use storage::context_storage::{ContextStorage, RecordKey, RecordValue};
use tezos_api::client::TezosStorageInitInfo;
use tezos_context::channel::ContextAction;
use tezos_wrapper::service::IpcEvtServer;
use tezos_encoding::hash::ChainId;

type SharedJoinHandle = Arc<Mutex<Option<JoinHandle<Result<(), Error>>>>>;

#[actor]
pub struct ContextListener {
    /// Thread where blocks are applied will run until this is set to `false`
    listener_run: Arc<AtomicBool>,
    /// Context event listener thread
    listener_thread: SharedJoinHandle,
}

pub type ContextListenerRef = ActorRef<ContextListenerMsg>;

impl ContextListener {
    pub fn actor(sys: &impl ActorRefFactory, rocks_db: Arc<rocksdb::DB>, mut event_server: IpcEvtServer,  init_info: &TezosStorageInitInfo, log: Logger) -> Result<ContextListenerRef, CreateError> {
        let listener_run = Arc::new(AtomicBool::new(true));
        let chain_id = init_info.chain_id.clone();
        let block_applier_thread = {
            let listener_run = listener_run.clone();

            thread::spawn(move || {
                let mut context_storage = ContextStorage::new(rocks_db);
                while listener_run.load(Ordering::Acquire) {
                    match listen_protocol_events(
                        &listener_run,
                        &mut event_server,
                        &mut context_storage,
                        &chain_id,
                        &log,
                    ) {
                        Ok(()) => debug!(log, "Context listener finished"),
                        Err(err) => warn!(log, "Timeout while waiting for context event connection"; "reason" => format!("{:?}", err)),
                    }
                }

                Ok(())
            })
        };

        let myself = sys.actor_of(
            Props::new_args(ContextListener::new, (listener_run, Arc::new(Mutex::new(Some(block_applier_thread))))),
            ContextListener::name())?;

        Ok(myself)
    }

    /// The `ContextListener` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "context-listener"
    }

    fn new((block_applier_run, block_applier_thread): (Arc<AtomicBool>, SharedJoinHandle)) -> Self {
        ContextListener {
            listener_run: block_applier_run,
            listener_thread: block_applier_thread,
        }
    }
}

impl Actor for ContextListener {
    type Msg = ContextListenerMsg;

    fn post_stop(&mut self) {
        self.listener_run.store(false, Ordering::Release);

        let _ = self.listener_thread.lock().unwrap()
            .take().expect("Thread join handle is missing")
            .join().expect("Failed to join context listener thread");
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

fn listen_protocol_events(
    apply_block_run: &AtomicBool,
    event_server: &mut IpcEvtServer,
    context_storage: &mut ContextStorage,
    chain_id: &ChainId,
    log: &Logger,
) -> Result<(), Error> {

    debug!(log, "Waiting for connection from protocol runner");
    let mut rx = event_server.accept()?;
    debug!(log, "Received connection from protocol runner. Starting to process context events.");

    let mut event_count = 0;
    while apply_block_run.load(Ordering::Acquire) {
        match rx.receive() {
            Ok(ContextAction::Shutdown) => break,
            Ok(msg) => {
                if event_count % 100 == 0 {
                    debug!(log, "Received protocol event"; "count" => event_count);
                }
                event_count += 1;
                match msg {
                    ContextAction::Set { context_hash: Some(context_hash), key, value } => {
                        let record_key = RecordKey::new(chain_id, 0, &context_hash, &key);
                        let record_value = RecordValue::new(Some(context_hash), key, value);
                        context_storage.put(&record_key, &record_value)?;
                    }
                    _ => (),
                }
            }
            Err(err) => {
                warn!(log, "Failed to receive event from protocol runner"; "reason" => format!("{:?}", err));
                break;
            },
        }

    }

    Ok(())
}

