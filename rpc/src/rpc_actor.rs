// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use getset::{Getters, Setters};
use riker::actors::*;
use slog::{Logger, warn};
use tokio::runtime::Handle;

use crypto::hash::ChainId;
use shell::shell_channel::{BlockApplied, CurrentMempoolState, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use storage::persistent::PersistentStorage;
use storage::StorageInitInfo;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_wrapper::TezosApiConnectionPool;

use crate::encoding::base_types::TimeStamp;
use crate::helpers::current_time_timestamp;
use crate::server::{RpcServiceEnvironment, spawn_server};

pub type RpcServerRef = ActorRef<RpcServerMsg>;

/// Thread safe reference to a shared RPC state
pub type RpcCollectedStateRef = Arc<RwLock<RpcCollectedState>>;

/// Represents various collected information about
/// internal state of the node.
#[derive(Getters, Setters)]
pub struct RpcCollectedState {
    #[get = "pub(crate)"]
    current_head: Option<BlockApplied>,
    #[get = "pub(crate)"]
    chain_id: ChainId,
    #[get = "pub(crate)"]
    current_mempool_state: Option<CurrentMempoolState>,
    #[get = "pub(crate)"]
    head_update_time: TimeStamp,
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
        tezos_readonly_api: Arc<TezosApiConnectionPool>,
        tezos_env: TezosEnvironmentConfiguration,
        network_version: NetworkVersion,
        init_storage_data: &StorageInitInfo) -> Result<RpcServerRef, CreateError> {
        let shared_state = Arc::new(RwLock::new(RpcCollectedState {
            current_head: load_current_head(persistent_storage, &init_storage_data.chain_id, &sys.log()),
            chain_id: init_storage_data.chain_id.clone(),
            current_mempool_state: None,
            head_update_time: current_time_timestamp(),
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
                tezos_readonly_api,
                &init_storage_data.genesis_block_header_hash,
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
                current_head_ref.head_update_time = current_time_timestamp();
            }
            ShellChannelMsg::MempoolStateChanged(result) => {
                let current_state = &mut *self.state.write().unwrap();
                current_state.current_mempool_state = Some(result);
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
                .get_with_json_data(&head.hash)
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