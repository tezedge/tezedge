// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Shell channel is used to transmit shutdown message to actors

use riker::actors::*;

/// Notify actors that system is about to shut down
#[derive(Clone, Debug)]
pub struct ShuttingDown;

/// Shell channel event message.
#[derive(Clone, Debug)]
pub enum ShellChannelMsg {
    ShuttingDown(ShuttingDown),
}

impl From<ShuttingDown> for ShellChannelMsg {
    fn from(msg: ShuttingDown) -> Self {
        ShellChannelMsg::ShuttingDown(msg)
    }
}

/// Represents various topics
pub enum ShellChannelTopic {
    /// Shutdown event
    ShellShutdown,
}

impl From<ShellChannelTopic> for Topic {
    fn from(evt: ShellChannelTopic) -> Self {
        match evt {
            ShellChannelTopic::ShellShutdown => Topic::from("shell.shutdown"),
        }
    }
}

/// This struct represents shell event bus where all shell events must be published.
pub struct ShellChannel(Channel<ShellChannelMsg>);

pub type ShellChannelRef = ChannelRef<ShellChannelMsg>;

impl ShellChannel {
    pub fn actor(fact: &impl ActorRefFactory) -> Result<ShellChannelRef, CreateError> {
        fact.actor_of::<ShellChannel>(ShellChannel::name())
    }

    fn name() -> &'static str {
        "shell-event-channel"
    }
}

type ChannelCtx<Msg> = Context<ChannelMsg<Msg>>;

impl ActorFactory for ShellChannel {
    fn create() -> Self {
        ShellChannel(Channel::default())
    }
}

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
