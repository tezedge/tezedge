// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use getset::{CopyGetters, Getters, Setters};
use riker::actors::*;
use slog::{error, info, warn, Logger};
use tokio::runtime::Handle;

use shell::mempool::CurrentMempoolStateStorageRef;
use shell::shell_channel::{ShellChannelMsg, ShellChannelRef};
use shell::subscription::subscribe_to_shell_new_current_head;
use shell_integration::ShellConnectorRef;
use storage::PersistentStorage;
use storage::{BlockHeaderWithHash, StorageInitInfo};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_wrapper::TezosApiConnectionPool;

use crate::helpers::{parse_chain_id, MAIN_CHAIN_ID};
use crate::server::{spawn_server, RpcServiceEnvironment};

pub type RpcServerRef = ActorRef<RpcServerMsg>;

/// Thread safe reference to a shared RPC state
pub type RpcCollectedStateRef = Arc<RwLock<RpcCollectedState>>;

/// Represents various collected information about
/// internal state of the node.
#[derive(CopyGetters, Getters, Setters)]
pub struct RpcCollectedState {
    #[get = "pub(crate)"]
    current_head: Arc<BlockHeaderWithHash>,
}

/// Actor responsible for managing HTTP REST API and server, and to share parts of inner actor
/// system with the server.
#[actor(ShellChannelMsg)]
pub struct RpcServer {
    state: RpcCollectedStateRef,
    env: Arc<RpcServiceEnvironment>,
}

impl RpcServer {
    pub fn name() -> &'static str {
        "rpc-server"
    }

    pub fn actor(
        sys: Arc<ActorSystem>,
        log: Logger,
        shell_channel: ShellChannelRef,
        shell_connector: ShellConnectorRef,
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
        hydrated_current_head_block: Arc<BlockHeaderWithHash>,
        tezedge_is_enabled: bool,
    ) -> Result<RpcServerRef, CreateError> {
        let shared_state = Arc::new(RwLock::new(RpcCollectedState {
            current_head: hydrated_current_head_block,
        }));

        let env = Arc::new(RpcServiceEnvironment::new(
            Arc::new(tokio_executor),
            shell_channel,
            shell_connector,
            tezos_env,
            network_version,
            persistent_storage,
            current_mempool_state_storage,
            tezos_readonly_api,
            tezos_readonly_prevalidation_api,
            tezos_without_context_api,
            init_storage_data.chain_id.clone(),
            shared_state.clone(),
            init_storage_data.context_stats_db_path.clone(),
            tezedge_is_enabled,
            log.clone(),
        ));

        let actor_ref = sys.actor_of_props::<RpcServer>(
            Self::name(),
            Props::new_args((shared_state, env.clone())),
        )?;

        // spawn RPC JSON server
        {
            let env_for_server = env.clone();

            env.tokio_executor().spawn(async move {
                info!(log, "Starting RPC server"; "address" => format!("{}", &rpc_listen_address));
                if let Err(e) = spawn_server(&rpc_listen_address, env_for_server).await {
                    error!(log, "HTTP Server encountered failure"; "error" => format!("{}", e));
                }
            });
        }

        Ok(actor_ref)
    }
}

impl ActorFactoryArgs<(RpcCollectedStateRef, Arc<RpcServiceEnvironment>)> for RpcServer {
    fn create_args((state, env): (RpcCollectedStateRef, Arc<RpcServiceEnvironment>)) -> Self {
        Self { state, env }
    }
}

impl Actor for RpcServer {
    type Msg = RpcServerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_new_current_head(self.env.shell_channel(), ctx.myself());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }
}

/// Timeout for RPCs warmup block time
const RPC_WARMUP_TIMEOUT: Duration = Duration::from_secs(3);

impl Receive<ShellChannelMsg> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        if let ShellChannelMsg::NewCurrentHead(_, block, is_bootstrapped) = msg {
            // prepare main chain_id
            let chain_id = parse_chain_id(MAIN_CHAIN_ID, &self.env).unwrap();

            // warm-up - calls where chain_id + block_hash
            if is_bootstrapped {
                let env = self.env.clone();
                let block = block.clone();
                let log = env.log().clone();
                self.env.tokio_executor().spawn(async move {
                    if let Err(err) = tokio::time::timeout(
                        RPC_WARMUP_TIMEOUT,
                        crate::services::cache_warm_up::warm_up_rpc_cache(chain_id, block, env),
                    )
                    .await
                    {
                        warn!(
                            log,
                            "RPC warmup timeout after {:?}: {:?}", RPC_WARMUP_TIMEOUT, err
                        );
                    }
                });
            }

            // update current head state
            match self.state.write() {
                Ok(mut current_head_ref) => {
                    current_head_ref.current_head = block;
                }
                Err(e) => {
                    warn!(self.env.log(), "Failed to update current head in RPC server env"; "reason" => format!("{}", e));
                }
            }
        }
    }
}
