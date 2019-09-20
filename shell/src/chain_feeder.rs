// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use std::time::Duration;

use failure::Error;
use log::warn;
use riker::actors::*;

use storage::{BlockMetaStorage, BlockStorage, OperationsStorage};

/// This command triggers feeding of completed blocks to the tezos protocol
#[derive(Clone, Debug)]
pub struct FeedChainToProtocol;

/// Feeds blocks and operations to the tezos protocol (ocaml code).
#[actor(FeedChainToProtocol)]
pub struct ChainFeeder {
    block_storage: BlockStorage,
    block_meta_storage: BlockMetaStorage,
    operations_storage: OperationsStorage,
}

pub type ChainFeederRef = ActorRef<ChainFeederMsg>;

impl ChainFeeder {
    pub fn actor(sys: &impl ActorRefFactory, rocks_db: Arc<rocksdb::DB>) -> Result<ChainFeederRef, CreateError> {
        sys.actor_of(
            Props::new_args(ChainFeeder::new, rocks_db),
            ChainFeeder::name())
    }

    /// The `ChainFeeder` is intended to serve as a singleton actor so that's why
    /// we won't support multiple names per instance.
    fn name() -> &'static str {
        "chain-feeder"
    }

    fn new(rocks_db: Arc<rocksdb::DB>) -> Self {
        ChainFeeder {
            block_storage: BlockStorage::new(rocks_db.clone()),
            block_meta_storage: BlockMetaStorage::new(rocks_db.clone()),
            operations_storage: OperationsStorage::new(rocks_db),
        }
    }

    fn feed_chain_to_protocol(&mut self) -> Result<(), Error> {
        unimplemented!()
    }
}

impl Actor for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        ctx.schedule::<Self::Msg, _>(
            Duration::from_secs(15),
            Duration::from_secs(60),
            ctx.myself(),
            None,
            FeedChainToProtocol.into());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<FeedChainToProtocol> for ChainFeeder {
    type Msg = ChainFeederMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: FeedChainToProtocol, _sender: Sender) {
        match self.feed_chain_to_protocol() {
            Ok(_) => (),
            Err(e) => warn!("Failed to feed chain to protocol: {:?}", e),
        }
    }
}
