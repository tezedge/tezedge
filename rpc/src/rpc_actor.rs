// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use getset::Getters;
use riker::actors::*;
use slog::warn;
use tokio::runtime::Runtime;

use shell::shell_channel::{BlockApplied, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use storage::{BlockHeaderWithHash, BlockStorageReader};
use storage::persistent::CommitLogs;
use tezos_api::client::TezosStorageInitInfo;
use tezos_encoding::hash::ChainId;

use crate::server::{RpcServiceEnvironment, spawn_server};

pub type RpcServerRef = ActorRef<RpcServerMsg>;

/// Thread safe reference to a shared RPC state
pub(crate) type RpcCollectedStateRef = Arc<RwLock<RpcCollectedState>>;

/// Represents various collected information about
/// internal state of the node.
#[derive(Getters)]
pub(crate) struct RpcCollectedState {
    #[get = "pub(crate)"]
    current_head: Option<BlockApplied>,
    #[get = "pub(crate)"]
    chain_id: ChainId
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

    fn new((shell_channel, state): (ShellChannelRef, RpcCollectedStateRef)) -> Self {
        Self { shell_channel, state }
    }

    pub fn actor(sys: &ActorSystem, shell_channel: ShellChannelRef, rpc_listen_address: SocketAddr, runtime: &Runtime, db: Arc<rocksdb::DB>, commit_logs: Arc<CommitLogs>, tezos_info: &TezosStorageInitInfo) -> Result<RpcServerRef, CreateError> {
        let shared_state = Arc::new(RwLock::new(RpcCollectedState {
            current_head: load_current_head(db.clone(), commit_logs.clone()).map(|block| block.into()),
            chain_id: tezos_info.chain_id.clone(),
        }));
        let actor_ref = sys.actor_of(
            Props::new_args(Self::new, (shell_channel, shared_state.clone())),
            Self::name(),
        )?;

        // spawn RPC JSON server
        {
            let server = spawn_server(&rpc_listen_address, RpcServiceEnvironment::new(sys.clone(), actor_ref.clone(), db, commit_logs, &tezos_info.genesis_block_header_hash, shared_state, sys.log()));
            let inner_log = sys.log();
            runtime.spawn(async move {
                if let Err(e) = server.await {
                    warn!(inner_log, "HTTP Server encountered failure"; "error" => format!("{}", e));
                }
            });
        }

        Ok(actor_ref)
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
            ShellChannelMsg::BlockApplied(block) => {
                let current_head_ref = &mut *self.state.write().unwrap();
                match &mut current_head_ref.current_head {
                    Some(current_head) => {
                        if current_head.level < block.level {
                            *current_head = block;
                        }
                    }
                    None => current_head_ref.current_head = Some(block)
                }
            }
            _ => (/* Not yet implemented, do nothing */),
        }
    }
}

/// Load local head (block with highest level) from dedicated storage
fn load_current_head(db: Arc<rocksdb::DB>, commit_logs: Arc<CommitLogs>) -> Option<BlockHeaderWithHash> {
    use storage::{BlockMetaStorage, BlockStorage, IteratorMode};
    use tezos_encoding::hash::BlockHash as RawBlockHash;

    let meta_storage = BlockMetaStorage::new(db.clone());
    let mut head: Option<RawBlockHash> = None;
    if let Ok(iter) = meta_storage.iter(IteratorMode::End) {
        let cur_level = -1;
        for (key, value) in iter {
            if let Ok(value) = value {
                if cur_level < value.level {
                    head = Some(key.unwrap())
                }
            }
        }
        if let Some(head) = head {
            let block_storage = BlockStorage::new(db.clone(), commit_logs);
            if let Ok(Some(head)) = block_storage.get(&head) {
                return Some(head);
            }
        }
    }
    None
}