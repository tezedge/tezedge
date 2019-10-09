// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;
use std::sync::Arc;
use std::time::Instant;

use riker::actor::*;
use rocksdb::DB;
use slog::warn;

use networking::p2p::binary_message::BinaryMessage;
use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic};

use crate::listener::events::{Event, EventPayloadStorage, EventStorage, EventType};

type NetworkListenerRef = ActorRef<NetworkChannelMsg>;

pub struct NetworkChannelListener {
    start: Instant,
    event_index: u64,
    record_storage: EventPayloadStorage,
    record_meta_storage: EventStorage,
    network_channel: NetworkChannelRef,
}

impl NetworkChannelListener {
    fn name() -> &'static str { "network-listener" }

    fn new((rocks_db, network_channel): (Arc<DB>, NetworkChannelRef)) -> Self {
        let record_meta_storage = EventStorage::new(rocks_db.clone());
        let event_index = record_meta_storage.count_events().unwrap_or_default() as u64;
        Self {
            start: Instant::now(),
            event_index,
            record_storage: EventPayloadStorage::new(rocks_db),
            record_meta_storage,
            network_channel,
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, rocks_db: Arc<DB>, network_channel: NetworkChannelRef) -> Result<NetworkListenerRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, (rocks_db, network_channel)),
            Self::name(),
        )
    }
}

impl Actor for NetworkChannelListener {
    type Msg = NetworkChannelMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.network_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, None);
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<BasicActorRef>) {
        // TryFrom<u128> for u64 fail iff value of converting u128 is bigger than u64::max_value()
        let timestamp = self.start.elapsed().as_micros().try_into().unwrap_or(u64::max_value());

        let (record_type, peer_id, record) = match msg {
            NetworkChannelMsg::PeerCreated(msg) => {
                (EventType::PeerCreated, msg.peer.name().to_string(), Vec::new())
            }
            NetworkChannelMsg::PeerBootstrapped(msg) => {
                (EventType::PeerBootstrapped, msg.peer.name().to_string(), msg.peer_id.into_bytes())
            }
            NetworkChannelMsg::PeerMessageReceived(msg) => {
                (EventType::PeerReceivedMessage, msg.peer.name().to_string(), msg.message.as_bytes().unwrap_or_default())
            }
        };

        let id = self.event_index;
        self.event_index += 1;

        if let Err(err) = self.record_meta_storage.put_event(id, &Event {
            record_type,
            timestamp,
            peer_id,
        }) {
            warn!(ctx.system.log(), "Failed to store meta for record"; "reason" => err);
        }
        if let Err(err) = self.record_storage.put_record(id, &record) {
            warn!(ctx.system.log(), "Failed to store record"; "reason" => err);
        }
    }
}