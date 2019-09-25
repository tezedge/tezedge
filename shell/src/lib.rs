// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use riker::actors::*;

use networking::p2p::network_channel::NetworkChannelTopic;

mod block_state;
mod operations_state;

pub mod shell_channel;
pub mod chain_feeder;
pub mod chain_manager;
pub mod peer_manager;

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

