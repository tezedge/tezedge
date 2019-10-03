// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use riker::actors::*;

use tezos_encoding::hash::BlockHash;

#[derive(Clone, Debug)]
pub struct BlockApplied {
    pub hash: BlockHash,
    pub level: i32,
}

#[derive(Clone, Debug)]
pub struct BlockReceived {
    pub hash: BlockHash,
    pub level: i32,
}

/// We have received message from another peer
#[derive(Clone, Debug)]
pub struct AllBlockOperationsReceived {
    pub hash: BlockHash,
    pub level: i32,
}

/// Shell channel event message.
#[derive(Clone, Debug)]
pub enum ShellChannelMsg {
    BlockApplied(BlockApplied),
    BlockReceived(BlockReceived),
    AllBlockOperationsReceived(AllBlockOperationsReceived),
}

impl From<BlockApplied> for ShellChannelMsg {
    fn from(msg: BlockApplied) -> Self {
        ShellChannelMsg::BlockApplied(msg)
    }
}

impl From<BlockReceived> for ShellChannelMsg {
    fn from(msg: BlockReceived) -> Self {
        ShellChannelMsg::BlockReceived(msg)
    }
}

impl From<AllBlockOperationsReceived> for ShellChannelMsg {
    fn from(msg: AllBlockOperationsReceived) -> Self {
        ShellChannelMsg::AllBlockOperationsReceived(msg)
    }
}

/// Represents various topics
pub enum ShellChannelTopic {
    /// Events generated from shell layer
    ShellEvents
}

impl From<ShellChannelTopic> for Topic {
    fn from(evt: ShellChannelTopic) -> Self {
        match evt {
            ShellChannelTopic::ShellEvents => Topic::from("shell.events")
        }
    }
}

/// This struct represents shell event bus where all shell events must be published.
pub struct ShellChannel(Channel<ShellChannelMsg>);

pub type ShellChannelRef = ChannelRef<ShellChannelMsg>;

impl ShellChannel {
    pub fn actor(fact: &impl ActorRefFactory) -> Result<ShellChannelRef, CreateError> {
        fact.actor_of(Props::new(ShellChannel::new), ShellChannel::name())
    }

    fn name() -> &'static str {
        "shell-event-channel"
    }

    fn new() -> Self {
        ShellChannel(Channel::new())
    }
}

type ChannelCtx<Msg> = Context<ChannelMsg<Msg>>;

impl Actor for ShellChannel {
    type Msg = ChannelMsg<ShellChannelMsg>;

    fn pre_start(&mut self, ctx: &ChannelCtx<ShellChannelMsg>) {
        self.0.pre_start(ctx);
    }

    fn recv(
        &mut self,
        ctx: &ChannelCtx<ShellChannelMsg>,
        msg: ChannelMsg<ShellChannelMsg>,
        sender: Sender,
    ) {
        self.0.receive(ctx, msg, sender);
    }
}
