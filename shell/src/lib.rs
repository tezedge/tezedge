use std::collections::HashMap;

use riker::actors::*;

use networking::p2p::network_channel::NetworkChannelTopic;

pub mod chain_manager;
pub mod peer_manager;

pub(crate) fn subscribe_to_actor_terminated<M, E>(event_channel: &ChannelRef<E>, myself: ActorRef<M>)
where
    M: Message,
    E: Message + Into<M>
{
    event_channel.tell(
        Subscribe {
            topic: SysTopic::ActorTerminated.into(),
            actor: Box::new(myself),
        }, None);
}

pub(crate) fn subscribe_to_network_events<M, E>(event_channel: &ChannelRef<E>, myself: ActorRef<M>)
where
    M: Message,
    E: Message + Into<M>
{
    event_channel.tell(
        Subscribe {
            actor: Box::new(myself),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, None);
}

pub(crate) fn remove_terminated_actor<T>(msg: &SystemEvent, actors: &mut HashMap<ActorUri, T>) {
    if let SystemEvent::ActorTerminated(evt) = msg {
        actors.remove(evt.actor.uri());
    }
}