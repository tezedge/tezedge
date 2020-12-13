// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use getset::{CopyGetters, Getters, Setters};
use riker::actors::*;
use slog::{Logger, warn};
use tokio::runtime::Handle;

use crypto::hash::ChainId;
use shell::mempool::CurrentMempoolStateStorageRef;
use shell::shell_channel::{BlockApplied, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use storage::context::TezedgeContext;
use storage::persistent::PersistentStorage;
use storage::StorageInitInfo;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_wrapper::TezosApiConnectionPool;

use crate::server::{RpcServiceEnvironment, spawn_server};

pub type RpcServerRef = ActorRef<RpcServerMsg>;

/// Thread safe reference to a shared RPC state
pub type RpcCollectedStateRef = Arc<RwLock<RpcCollectedState>>;

/// Represents various collected information about
/// internal state of the node.
#[derive(CopyGetters, Getters, Setters)]
pub struct RpcCollectedState {
    #[get = "pub(crate)"]
    current_head: Option<BlockApplied>,
    #[get_copy = "pub(crate)"]
    is_sandbox: bool,
}

/// Actor responsible for managing HTTP REST API and server, and to share parts of inner actor
/// system with the server.
#[actor(ShellChannelMsg)]
pub struct RpcServer {
    shell_channel: ShellChannelRef,
    state: RpcCollectedStateRef,
}

impl RpcServer {
    pub fn name() -> &'static str { "rpc-server" }

    pub fn actor(
        sys: &ActorSystem,
        shell_channel: ShellChannelRef,
        rpc_listen_address: SocketAddr,
        tokio_executor: &Handle,
        persistent_storage: &PersistentStorage,
        current_mempool_state_storage: CurrentMempoolStateStorageRef,
        tezedge_context: &TezedgeContext,
        tezos_readonly_api: Arc<TezosApiConnectionPool>,
        tezos_readonly_prevalidation_api: Arc<TezosApiConnectionPool>,
        tezos_without_context_api: Arc<TezosApiConnectionPool>,
        tezos_env: TezosEnvironmentConfiguration,
        network_version: Arc<NetworkVersion>,
        init_storage_data: &StorageInitInfo,
        is_sandbox: bool,
    ) -> Result<RpcServerRef, CreateError> {
        let shared_state = Arc::new(RwLock::new(RpcCollectedState {
            current_head: load_current_head(persistent_storage, &init_storage_data.chain_id, &sys.log()),
            is_sandbox,
        }));
        let actor_ref = sys.actor_of_props::<RpcServer>(
            Self::name(),
            Props::new_args((shell_channel.clone(), shared_state.clone())),
        )?;

        // spawn RPC JSON server
        {
            let env = RpcServiceEnvironment::new(
                sys.clone(),
                actor_ref.clone(),
                shell_channel,
                tezos_env,
                network_version,
                persistent_storage,
                current_mempool_state_storage,
                tezedge_context,
                tezos_readonly_api,
                tezos_readonly_prevalidation_api,
                tezos_without_context_api,
                init_storage_data.chain_id.clone(),
                init_storage_data.genesis_block_header_hash.clone(),
                shared_state,
                &sys.log(),
            );
            let inner_log = sys.log();

            tokio_executor.spawn(async move {
                if let Err(e) = spawn_server(&rpc_listen_address, env).await {
                    warn!(inner_log, "HTTP Server encountered failure"; "error" => format!("{}", e));
                }
            });
        }

        Ok(actor_ref)
    }
}

impl ActorFactoryArgs<(ShellChannelRef, RpcCollectedStateRef)> for RpcServer {
    fn create_args((shell_channel, state): (ShellChannelRef, RpcCollectedStateRef)) -> Self {
        Self { shell_channel, state }
    }
}

impl Actor for RpcServer {
    type Msg = RpcServerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.shell_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, ctx.myself().into());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ShellChannelMsg> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match msg {
            ShellChannelMsg::NewCurrentHead(_, block) => {
                let current_head_ref = &mut *self.state.write().unwrap();
                current_head_ref.current_head = Some(block);
            }
            _ => (/* Not yet implemented, do nothing */),
        }
    }
}

/// Load local head (block with highest level) from dedicated storage
fn load_current_head(persistent_storage: &PersistentStorage, chain_id: &ChainId, log: &Logger) -> Option<BlockApplied> {
    use storage::{BlockStorage, BlockStorageReader, ChainMetaStorage, StorageError};
    use storage::chain_meta_storage::ChainMetaStorageReader;

    let chain_meta_storage = ChainMetaStorage::new(persistent_storage);
    match chain_meta_storage.get_current_head(chain_id) {
        Ok(Some(head)) => {
            let block_applied = BlockStorage::new(persistent_storage)
                .get_with_json_data(head.block_hash())
                .and_then(|data| data.map(|(block, json)| BlockApplied::new(block, json)).ok_or(StorageError::MissingKey));
            match block_applied {
                Ok(block) => Some(block),
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
