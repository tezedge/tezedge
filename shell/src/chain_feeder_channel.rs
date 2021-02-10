// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This channel is used to transmit [`BlockApplied`] message for [`peer_branch_bootstrappers`]

use std::sync::Arc;

use riker::actors::*;

use crypto::hash::BlockHash;

/// Channel event message.
#[derive(Clone, Debug)]
pub enum ChainFeederChannelMsg {
    BlockApplied(Arc<BlockHash>),
}

/// Represents various topics
pub enum ChainFeederChannelTopic {
    /// Dedicated channel for block applied
    BlockApplied,
}

impl From<ChainFeederChannelTopic> for Topic {
    fn from(evt: ChainFeederChannelTopic) -> Self {
        match evt {
            ChainFeederChannelTopic::BlockApplied => Topic::from("chain_feeder.block_applied"),
        }
    }
}

/// This struct represents shell event bus where all shell events must be published.
pub struct ChainFeederChannel(Channel<ChainFeederChannelMsg>);

pub type ChainFeederChannelRef = ChannelRef<ChainFeederChannelMsg>;

impl ChainFeederChannel {
    pub fn actor(fact: &impl ActorRefFactory) -> Result<ChainFeederChannelRef, CreateError> {
        fact.actor_of::<ChainFeederChannel>(ChainFeederChannel::name())
    }

    fn name() -> &'static str {
        "chain-feeder-event-channel"
    }
}

type ChannelCtx<Msg> = Context<ChannelMsg<Msg>>;

impl ActorFactory for ChainFeederChannel {
    fn create() -> Self {
        ChainFeederChannel(Channel::default())
    }
}

impl Actor for ChainFeederChannel {
    type Msg = ChannelMsg<ChainFeederChannelMsg>;

    fn pre_start(&mut self, ctx: &ChannelCtx<ChainFeederChannelMsg>) {
        self.0.pre_start(ctx);
    }

    fn recv(
        &mut self,
        ctx: &ChannelCtx<ChainFeederChannelMsg>,
        msg: ChannelMsg<ChainFeederChannelMsg>,
        sender: Sender,
    ) {
        self.0.receive(ctx, msg, sender);
    }
}

#[inline]
pub fn subscribe_to_chain_feeder_block_applied_channel<M, E>(
    channel: &ChannelRef<E>,
    myself: ActorRef<M>,
) where
    M: Message,
    E: Message + Into<M>,
{
    channel.tell(
        Subscribe {
            actor: Box::new(myself),
            topic: ChainFeederChannelTopic::BlockApplied.into(),
        },
        None,
    );
}
