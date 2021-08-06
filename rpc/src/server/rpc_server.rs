// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::ChainId;
use shell::mempool::{CurrentMempoolStateStorageRef, MempoolPrevalidatorFactory};
use shell::subscription::*;
use shell::tezedge_state_manager::ProposerHandle;
use slog::{error, info, warn, Logger};
use std::net::SocketAddr;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use storage::PersistentStorage;
use storage::{BlockHeaderWithHash, StorageInitInfo};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_wrapper::TezosApiConnectionPool;
use tokio::runtime::Handle;

use crate::server::{spawn_server, RpcCollectedState, RpcServiceEnvironment};

pub enum NotifyRpcServerMsg {
    /// If tezedge state machine resolved new current head for chain
    NewCurrentHead(NewCurrentHeadNotificationRef),
    Shutdown,
}

enum RpcServerThreadHandle {
    Running(std::thread::JoinHandle<()>, tokio::task::JoinHandle<()>),
    NotRunning(mpsc::Receiver<NotifyRpcServerMsg>),
}

#[derive(Debug)]
pub enum RpcServerSpawnError {
    IoError(std::io::Error),
}

/// RpcServer is responsible for managing HTTP REST API and server, and to share parts of inner actor
/// system with the server.
pub struct RpcServer {
    env: Arc<RpcServiceEnvironment>,
    rpc_listen_address: SocketAddr,
    log: Logger,
    notify_rpc_server_tx: mpsc::SyncSender<NotifyRpcServerMsg>,
    rpc_server_thread_handle: Option<RpcServerThreadHandle>,
}

impl RpcServer {
    const NOTIFY_QUEUE_MAX_CAPACITY: usize = 50_000;

    pub fn new(
        mempool_prevalidator_factory: Arc<MempoolPrevalidatorFactory>,
        log: Logger,
        proposer: ProposerHandle,
        rpc_listen_address: SocketAddr,
        tokio_executor: Handle,
        persistent_storage: &PersistentStorage,
        current_mempool_state_storage: CurrentMempoolStateStorageRef,
        tezos_readonly_api: Arc<TezosApiConnectionPool>,
        tezos_readonly_prevalidation_api: Arc<TezosApiConnectionPool>,
        tezos_without_context_api: Arc<TezosApiConnectionPool>,
        tezos_env: TezosEnvironmentConfiguration,
        network_version: Arc<NetworkVersion>,
        init_storage_data: &StorageInitInfo,
        tezedge_is_enabled: bool,
    ) -> Self {
        let shared_state = Arc::new(RwLock::new(RpcCollectedState {
            current_head: load_current_head(persistent_storage, &init_storage_data.chain_id, &log),
        }));

        let env = Arc::new(RpcServiceEnvironment::new(
            mempool_prevalidator_factory,
            Arc::new(tokio_executor),
            proposer,
            tezos_env,
            network_version,
            persistent_storage,
            current_mempool_state_storage,
            tezos_readonly_api,
            tezos_readonly_prevalidation_api,
            tezos_without_context_api,
            init_storage_data.chain_id.clone(),
            init_storage_data.genesis_block_header_hash.clone(),
            shared_state,
            init_storage_data.context_stats_db_path.clone(),
            tezedge_is_enabled,
            log.clone(),
        ));

        let (notify_rpc_server_tx, notify_rpc_server_rx) =
            mpsc::sync_channel(Self::NOTIFY_QUEUE_MAX_CAPACITY);

        Self {
            env,
            rpc_listen_address,
            log,
            notify_rpc_server_tx,
            rpc_server_thread_handle: Some(RpcServerThreadHandle::NotRunning(notify_rpc_server_rx)),
        }
    }

    pub fn start(&mut self) -> Result<(), RpcServerSpawnError> {
        if let Some(RpcServerThreadHandle::NotRunning(notify_rpc_server_rx)) =
            self.rpc_server_thread_handle.take()
        {
            // 1. spawn mpsc queue receiver for notifications
            let notify_rpc_server_listener_thread = {
                let env = self.env.clone();

                std::thread::Builder::new()
                    .name("rpc-notifier".to_owned())
                    .spawn(move || {
                        run_notify_rpc_server_listener(notify_rpc_server_rx, env);
                    })
                    .map_err(RpcServerSpawnError::IoError)?
            };

            // 2. spawn RPC JSON server
            let rpc_server_thread_handle = {
                let rpc_listen_address = self.rpc_listen_address;
                let log = self.log.clone();
                let env_for_server = self.env.clone();

                self.env.tokio_executor().spawn(async move {
                    info!(log, "Starting RPC server"; "address" => format!("{}", &rpc_listen_address));
                    if let Err(e) = spawn_server(&rpc_listen_address, env_for_server).await {
                        error!(log, "HTTP Server encountered failure"; "error" => format!("{}", e));
                    }
                })
            };

            self.rpc_server_thread_handle = Some(RpcServerThreadHandle::Running(
                notify_rpc_server_listener_thread,
                rpc_server_thread_handle,
            ));
        }

        Ok(())
    }

    fn notify_rpc_server_tx(&self) -> mpsc::SyncSender<NotifyRpcServerMsg> {
        self.notify_rpc_server_tx.clone()
    }
}

/// Timeout for RPCs warmup block time
const RPC_WARMUP_TIMEOUT: Duration = Duration::from_secs(3);

fn run_notify_rpc_server_listener(
    rx: mpsc::Receiver<NotifyRpcServerMsg>,
    env: Arc<RpcServiceEnvironment>,
) {
    // Read and handle messages incoming from actor system or `PeerManager`.
    loop {
        match rx.recv() {
            Ok(NotifyRpcServerMsg::NewCurrentHead(notification)) => {
                // warm-up - calls where chain_id + block_hash
                if notification.is_bootstrapped {
                    let block = notification.block.clone();
                    let chain_id = notification.chain_id.as_ref().clone();
                    let inner_env = env.clone();
                    env.tokio_executor().spawn(async move {
                        if let Err(err) = tokio::time::timeout(
                            RPC_WARMUP_TIMEOUT,
                            crate::services::cache_warm_up::warm_up_rpc_cache(
                                chain_id, block, &inner_env,
                            ),
                        )
                        .await
                        {
                            warn!(
                                inner_env.log(),
                                "RPC warmup timeout after {:?}: {:?}", RPC_WARMUP_TIMEOUT, err
                            );
                        }
                    });
                }

                match env.state().write() {
                    Ok(mut current_head_ref) => {
                        current_head_ref.current_head = Some(notification.block.clone());
                    }
                    Err(e) => {
                        warn!(env.log(), "Failed to update current head in RPC server env"; "reason" => format!("{}", e));
                    }
                }
            }
            Ok(NotifyRpcServerMsg::Shutdown) => {
                info!(env.log(), "Rpc notification listener finished");
                return;
            }
            Err(mpsc::RecvError) => {
                info!(
                    env.log(),
                    "Rpc notification listener finished (by disconnect)"
                );
                return;
            }
        }
    }
}

/// Load local head (block with highest level) from dedicated storage
fn load_current_head(
    persistent_storage: &PersistentStorage,
    chain_id: &ChainId,
    log: &Logger,
) -> Option<Arc<BlockHeaderWithHash>> {
    use storage::chain_meta_storage::ChainMetaStorageReader;
    use storage::{BlockStorage, BlockStorageReader, ChainMetaStorage, StorageError};

    let chain_meta_storage = ChainMetaStorage::new(persistent_storage);
    match chain_meta_storage.get_current_head(chain_id) {
        Ok(Some(head)) => {
            let block_applied = BlockStorage::new(persistent_storage)
                .get(head.block_hash())
                .and_then(|data| {
                    data.ok_or_else(|| StorageError::MissingKey {
                        when: "load_current_head".into(),
                    })
                });
            match block_applied {
                Ok(block) => Some(Arc::new(block)),
                Err(e) => {
                    warn!(log, "Error reading current head detail from database."; "reason" => format!("{}", e));
                    None
                }
            }
        }
        Ok(None) => None,
        Err(e) => {
            warn!(log, "Error reading current head from database."; "reason" => format!("{}", e));
            None
        }
    }
}

impl NotifyCallbackRegistrar<NewCurrentHeadNotificationRef> for RpcServer {
    fn subscribe(&self, notifier: &mut Notifier<NewCurrentHeadNotificationRef>) {
        notifier.register(
            Box::new(RpcServerNotifyCallback(self.notify_rpc_server_tx())),
            "rpc-server-new-current-head-listener".to_string(),
        );
    }
}

struct RpcServerNotifyCallback(mpsc::SyncSender<NotifyRpcServerMsg>);

impl NotifyCallback<NewCurrentHeadNotificationRef> for RpcServerNotifyCallback {
    fn notify(
        &self,
        notification: NewCurrentHeadNotificationRef,
    ) -> Result<(), SendNotificationError> {
        self.0
            .send(NotifyRpcServerMsg::NewCurrentHead(notification))
            .map_err(|e| SendNotificationError {
                reason: format!("{}", e),
            })
    }

    fn shutdown(&self) -> Result<(), ShutdownCallbackError> {
        self.0
            .try_send(NotifyRpcServerMsg::Shutdown)
            .map_err(|e| ShutdownCallbackError {
                reason: format!("{}", e),
            })
    }
}
