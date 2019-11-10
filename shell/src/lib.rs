// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use riker::actors::*;

use networking::p2p::network_channel::NetworkChannelTopic;

use crate::shell_channel::ShellChannelTopic;

mod collections;
mod state;

pub mod shell_channel;
pub mod chain_feeder;
pub mod context_listener;
pub mod chain_manager;
pub mod peer_manager;

#[inline]
pub(crate) fn subscribe_to_actor_terminated<M, E>(sys_channel: &ChannelRef<E>, myself: ActorRef<M>)
where
    M: Message,
    E: Message + Into<M>
{
    sys_channel.tell(
        Subscribe {
            topic: SysTopic::ActorTerminated.into(),
            actor: Box::new(myself),
        }, None);
}

#[inline]
pub(crate) fn subscribe_to_network_events<M, E>(network_channel: &ChannelRef<E>, myself: ActorRef<M>)
where
    M: Message,
    E: Message + Into<M>
{
    network_channel.tell(
        Subscribe {
            actor: Box::new(myself),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, None);
}

#[inline]
pub(crate) fn subscribe_to_shell_events<M, E>(shell_channel: &ChannelRef<E>, myself: ActorRef<M>)
where
    M: Message,
    E: Message + Into<M>
{
    shell_channel.tell(
        Subscribe {
            actor: Box::new(myself),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, None);
}

