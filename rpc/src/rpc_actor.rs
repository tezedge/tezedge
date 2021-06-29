// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use getset::{CopyGetters, Getters, Setters};
use riker::actors::*;
use slog::{error, info, warn, Logger};
use tokio::runtime::Handle;

use crypto::hash::ChainId;
use shell::mempool::CurrentMempoolStateStorageRef;
use shell::shell_channel::{ShellChannelMsg, ShellChannelRef};
use shell::subscription::subscribe_to_shell_new_current_head;
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
    current_head: Option<Arc<BlockHeaderWithHash>>,
}

/// Actor responsible for managing HTTP REST API and server, and to share parts of inner actor
/// system with the server.
#[actor(ShellChannelMsg)]
pub struct RpcServer {
    shell_channel: ShellChannelRef,
    state: RpcCollectedStateRef,
    env: Arc<RpcServiceEnvironment>,
}

impl RpcServer {
    pub fn name() -> &'static str {
        "rpc-server"
    }

    pub fn actor(
        sys: &ActorSystem,
        shell_channel: ShellChannelRef,
        rpc_listen_address: SocketAddr,
        tokio_executor: &Handle,
        persistent_storage: &PersistentStorage,
        current_mempool_state_storage: CurrentMempoolStateStorageRef,
        tezos_readonly_api: Arc<TezosApiConnectionPool>,
        tezos_readonly_prevalidation_api: Arc<TezosApiConnectionPool>,
        tezos_without_context_api: Arc<TezosApiConnectionPool>,
        tezos_env: TezosEnvironmentConfiguration,
        network_version: Arc<NetworkVersion>,
        init_storage_data: &StorageInitInfo,
        tezedge_is_enabled: bool,
    ) -> Result<RpcServerRef, CreateError> {
        let shared_state = Arc::new(RwLock::new(RpcCollectedState {
            current_head: load_current_head(
                persistent_storage,
                &init_storage_data.chain_id,
                &sys.log(),
            ),
        }));

        let env = Arc::new(RpcServiceEnvironment::new(
            sys.clone(),
            shell_channel.clone(),
            tezos_env,
            network_version,
            persistent_storage,
            current_mempool_state_storage,
            tezos_readonly_api,
            tezos_readonly_prevalidation_api,
            tezos_without_context_api,
            init_storage_data.chain_id.clone(),
            init_storage_data.genesis_block_header_hash.clone(),
            shared_state.clone(),
            init_storage_data.context_stats_db_path.clone(),
            tezedge_is_enabled,
            &sys.log(),
        ));

        let actor_ref = sys.actor_of_props::<RpcServer>(
            Self::name(),
            Props::new_args((shell_channel, shared_state, env.clone())),
        )?;

        // spawn RPC JSON server
        {
            let inner_log = sys.log();

            tokio_executor.spawn(async move {
                info!(inner_log, "Starting RPC server"; "address" => format!("{}", &rpc_listen_address));
                if let Err(e) = spawn_server(&rpc_listen_address, env).await {
                    error!(inner_log, "HTTP Server encountered failure"; "error" => format!("{}", e));
                }
            });
        }

        Ok(actor_ref)
    }
}

impl
    ActorFactoryArgs<(
        ShellChannelRef,
        RpcCollectedStateRef,
        Arc<RpcServiceEnvironment>,
    )> for RpcServer
{
    fn create_args(
        (shell_channel, state, env): (
            ShellChannelRef,
            RpcCollectedStateRef,
            Arc<RpcServiceEnvironment>,
        ),
    ) -> Self {
        Self {
            shell_channel,
            state,
            env,
        }
    }
}

impl Actor for RpcServer {
    type Msg = RpcServerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_new_current_head(&self.shell_channel, ctx.myself());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ShellChannelMsg> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        if let ShellChannelMsg::NewCurrentHead(_, block) = msg {
            // prepare main chain_id
            let chain_id = parse_chain_id(MAIN_CHAIN_ID, &self.env).unwrap();
            // warm-up - calls where chain_id + block_hash
            let _ = crate::services::base_services::get_block_metadata(
                &chain_id,
                &block.hash,
                &self.env,
            );
            let _ = crate::services::base_services::get_additional_data(
                &chain_id,
                &block.hash,
                &self.env.persistent_storage(),
            );
            let _ = crate::services::base_services::get_block(&chain_id, &block.hash, &self.env);
            let _ = crate::services::base_services::get_block_operations_metadata(
                chain_id.clone(),
                &block.hash,
                &self.env,
            );
            let _ = crate::services::base_services::get_block_operation_hashes(
                chain_id.clone(),
                &block.hash,
                &self.env,
            );
            let _ = crate::services::base_services::get_block_protocols(
                &chain_id,
                &block.hash,
                &self.env.persistent_storage(),
            );
            let _ = crate::services::base_services::live_blocks(
                chain_id.clone(),
                block.hash.clone(),
                &self.env,
            );
            let _ = crate::services::base_services::get_block_shell_header(
                chain_id.clone(),
                block.hash.clone(),
                &self.env.persistent_storage(),
            );
            let _ = crate::services::base_services::get_block_header(
                chain_id.clone(),
                block.hash.clone(),
                &self.env.persistent_storage(),
            );
            let current_head_ref = &mut *self.state.write().unwrap();
            current_head_ref.current_head = Some(block);
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
                .and_then(|data| data.ok_or(StorageError::MissingKey));
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
