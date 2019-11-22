// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::sync::Arc;

use riker::{
    actors::*,
};
use slog::warn;
use tokio::runtime::Runtime;

use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic};
use shell::shell_channel::{BlockApplied, ShellChannelMsg, ShellChannelRef, ShellChannelTopic};
use storage::{BlockHeaderWithHash, BlockStorageReader};
use tezos_api::client::TezosStorageInitInfo;
use tezos_encoding::hash::{BlockHash, ChainId, HashEncoding, HashType, ProtocolHash};

use crate::{
    server::{spawn_server, control_msg::*},
};
use crate::helpers::FullBlockInfo;

pub type RpcServerRef = ActorRef<RpcServerMsg>;

/// Actor responsible for managing HTTP REST API and server, and to share parts of inner actor
/// system with the server.
#[actor(NetworkChannelMsg, ShellChannelMsg, GetCurrentHead, GetFullCurrentHead, GetBlocks)]
pub struct RpcServer {
    network_channel: NetworkChannelRef,
    shell_channel: ShellChannelRef,
    // Stats
    chain_id: ChainId,
    _supported_protocols: Vec<ProtocolHash>,
    genesis_hash: BlockHash,
    current_head: Option<BlockApplied>,
    db: Arc<rocksdb::DB>,
}

impl RpcServer {
    pub fn name() -> &'static str { "rpc-server" }

    fn new((network_channel, shell_channel, db, chain_id, genesis_hash, supported_protocols): (NetworkChannelRef, ShellChannelRef, Arc<rocksdb::DB>, ChainId, BlockHash, Vec<ProtocolHash>)) -> Self {
        let current_head = Self::load_current_head(db.clone()).map(|block| block.into());

        Self {
            network_channel,
            shell_channel,
            chain_id,
            genesis_hash,
            _supported_protocols: supported_protocols,
            current_head,
            db,
        }
    }

    pub fn actor(sys: &ActorSystem, network_channel: NetworkChannelRef, shell_channel: ShellChannelRef, addr: SocketAddr, runtime: &Runtime, db: Arc<rocksdb::DB>, tezos_info: &TezosStorageInitInfo) -> Result<RpcServerRef, CreateError> {
        let chain_id = tezos_info.chain_id.clone();
        let genesis_hash = tezos_info.genesis_block_header_hash.clone();
        let protocols = tezos_info.supported_protocol_hashes.clone();

        let ret = sys.actor_of(
            Props::new_args(Self::new, (network_channel, shell_channel, db, chain_id, genesis_hash, protocols)),
            Self::name(),
        )?;

        let server = spawn_server(&addr, sys.clone(), ret.clone());
        let inner_log = sys.log();
        runtime.spawn(async move {
            if let Err(e) = server.await {
                warn!(inner_log, "HTTP Server encountered failure"; "error" => format!("{}", e));
            }
        });
        Ok(ret)
    }

    /// Load local head (block with highest level) from dedicated storage
    fn load_current_head(db: Arc<rocksdb::DB>) -> Option<BlockHeaderWithHash> {
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
                let block_storage = BlockStorage::new(db.clone());
                if let Ok(Some(head)) = block_storage.get(&head) {
                    return Some(head);
                }
            }
        }
        None
    }
}

impl Actor for RpcServer {
    type Msg = RpcServerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.network_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, ctx.myself().into());

        self.shell_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, ctx.myself().into());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<NetworkChannelMsg> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: NetworkChannelMsg, _sender: Sender) {
        /* Not yet implemented, do nothing */
    }
}

impl Receive<ShellChannelMsg> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match msg {
            ShellChannelMsg::BlockApplied(data) => {
                if let Some(ref current_head) = self.current_head {
                    if current_head.level < data.level {
                        self.current_head = Some(data);
                    }
                } else {
                    self.current_head = Some(data);
                }
            }
            _ => (/* Not yet implemented, do nothing */),
        }
    }
}

impl Receive<GetCurrentHead> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: GetCurrentHead, sender: Sender) {
        if let GetCurrentHead::Request = msg {
            if let Some(sender) = sender {
                let me: Option<BasicActorRef> = ctx.myself().into();
                if sender.try_tell(GetCurrentHead::Response(self.current_head.clone()), me).is_err() {
                    warn!(ctx.system.log(), "Failed to send response for GetCurrentHead");
                }
            }
        }
    }
}

impl Receive<GetFullCurrentHead> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: GetFullCurrentHead, sender: Sender) {
        use storage::{OperationsStorage, operations_storage::OperationsStorageReader};

        if let GetFullCurrentHead::Request = msg {
            if let Some(sender) = sender {
                let me: Option<BasicActorRef> = ctx.myself().into();
                let current_head = self.current_head.clone();
                let resp = GetFullCurrentHead::Response(if let Some(head) = current_head {
                    let ops_storage = OperationsStorage::new(self.db.clone());
                    let _ops = ops_storage.get_operations(&head.hash).unwrap_or_default();
                    let mut head: FullBlockInfo = head.into();
                    head.chain_id = HashEncoding::new(HashType::ChainId).bytes_to_string(&self.chain_id);
                    Some(head)
                } else {
                    None
                });

                if sender.try_tell(resp, me).is_err() {
                    warn!(ctx.system.log(), "Failed to send response for GetFullCurrentHead");
                }
            }
        }
    }
}

impl Receive<GetBlocks> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: GetBlocks, sender: Sender) {
        use storage::{BlockMetaStorage, BlockStorage};

        if let GetBlocks::Request { block_hash, limit } = msg {
            if let Some(sender) = sender {
                let block_storage = BlockStorage::new(self.db.clone());
                let block_meta_storage = BlockMetaStorage::new(self.db.clone());

                let mut resp_data = Vec::with_capacity(limit);
                // get starting block hash or use genesis hash
                let mut block_hash = block_hash.and_then(|hash| HashEncoding::new(HashType::BlockHash).string_to_bytes(&hash).ok()).unwrap_or(self.genesis_hash.clone());

                for _ in 0..limit {
                    match block_meta_storage.get(&block_hash) {
                        Ok(Some(meta)) => match block_storage.get(&block_hash) {
                                Ok(Some(block)) => {
                                    resp_data.push(block.into());
                                    match meta.successor {
                                        Some(successor) => block_hash = successor,
                                        None => break
                                    }
                                },
                                Err(err) => {
                                    warn!(ctx.system.log(), "Failed to retrieve block from storage"; "reason" => err);
                                    break
                                },
                                _ => break
                        }
                        Err(err) => {
                            warn!(ctx.system.log(), "Failed to retrieve block metadata from storage"; "reason" => err);
                            break
                        },
                        _ => break
                    }
                }

                if sender.try_tell(GetBlocks::Response(resp_data), Some(ctx.myself().into())).is_err() {
                    warn!(ctx.system.log(), "Failed to send response for GetBlocks");
                }
            }
        }
    }
}