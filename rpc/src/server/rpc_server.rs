// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use shell_automaton::service::rpc_service::RpcShellAutomatonSender;
use shell_integration::notifications::*;
use shell_integration::*;
use slog::{error, info, warn, Logger};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use storage::PersistentStorage;
use storage::{BlockHeaderWithHash, StorageInitInfo};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_protocol_ipc_client::ProtocolRunnerApi;
use tokio::runtime::Handle;

use crate::server::{spawn_server, RpcCollectedState, RpcServiceEnvironment};
use crate::RpcServiceEnvironmentRef;

enum RpcServerThreadHandle {
    Running(tokio::task::JoinHandle<()>),
    NotRunning,
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
    rpc_server_thread_handle: Option<RpcServerThreadHandle>,
}

impl RpcServer {
    pub fn new(
        log: Logger,
        shell_connector: ShellConnectorRef,
        shell_automaton_channel: RpcShellAutomatonSender,
        rpc_listen_address: SocketAddr,
        tokio_executor: Handle,
        persistent_storage: &PersistentStorage,
        tezos_protocol_api: Arc<ProtocolRunnerApi>,
        tezos_env: TezosEnvironmentConfiguration,
        network_version: Arc<NetworkVersion>,
        init_storage_data: &StorageInitInfo,
        hydrated_current_head_block: Arc<BlockHeaderWithHash>,
        tezedge_is_enabled: bool,
    ) -> Self {
        let shared_state = Arc::new(RwLock::new(RpcCollectedState {
            current_head: hydrated_current_head_block,
            streams: HashMap::new(),
        }));

        let env = Arc::new(RpcServiceEnvironment::new(
            Arc::new(tokio_executor),
            shell_connector,
            shell_automaton_channel,
            tezos_env,
            network_version,
            persistent_storage,
            tezos_protocol_api,
            init_storage_data.chain_id.clone(),
            shared_state,
            init_storage_data.context_stats_db_path.clone(),
            tezedge_is_enabled,
            log.clone(),
        ));

        Self {
            env,
            rpc_listen_address,
            log,
            rpc_server_thread_handle: Some(RpcServerThreadHandle::NotRunning),
        }
    }

    pub fn start(&mut self) -> Result<(), RpcServerSpawnError> {
        if let Some(RpcServerThreadHandle::NotRunning) = self.rpc_server_thread_handle.take() {
            // spawn RPC JSON server
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

            self.rpc_server_thread_handle =
                Some(RpcServerThreadHandle::Running(rpc_server_thread_handle));
        }

        Ok(())
    }

    pub fn rpc_env(&self) -> RpcServiceEnvironmentRef {
        self.env.clone()
    }
}

/// Timeout for RPCs warmup block time
const RPC_WARMUP_TIMEOUT: Duration = Duration::from_secs(3);

/// Callback, which can be triggered, when need to update shared state of rpc's state
pub fn handle_notify_rpc_server_msg(
    env: &Arc<RpcServiceEnvironment>,
    notification: NewCurrentHeadNotificationRef,
) {
    // warm-up - calls where chain_id + block_hash
    if notification.is_bootstrapped {
        let block = notification.block.clone();
        let chain_id = notification.chain_id.as_ref().clone();
        let inner_env = env.clone();
        env.tokio_executor().spawn(async move {
            if let Err(err) = tokio::time::timeout(
                RPC_WARMUP_TIMEOUT,
                crate::services::cache_warm_up::warm_up_rpc_cache(chain_id, block, &inner_env),
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
            current_head_ref.current_head = notification.block.clone();
            current_head_ref.wake_up_all_streams();
        }
        Err(e) => {
            warn!(env.log(), "Failed to update current head in RPC server env"; "reason" => format!("{}", e));
        }
    }
}
