use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic};
use riker::actor::*;
use crate::listener::records::{RecordStorage, RecordMetaStorage, RecordMeta, RecordType};
use std::sync::Arc;
use rocksdb::DB;
use std::time::Instant;
use networking::p2p::binary_message::BinaryMessage;

type NetworkListenerRef = ActorRef<NetworkChannelMsg>;

pub struct NetworkListener {
    start: Instant,
    record_storage: RecordStorage,
    record_meta_storage: RecordMetaStorage,
    network_channel: NetworkChannelRef,
}

impl NetworkListener {
    fn name() -> &'static str { "network-listener" }

    fn new((rocks_db, network_channel): (Arc<DB>, NetworkChannelRef)) -> Self {
        Self {
            start: Instant::now(),
            record_storage: RecordStorage::new(rocks_db.clone()),
            record_meta_storage: RecordMetaStorage::new(rocks_db),
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

impl Actor for NetworkListener {
    type Msg = NetworkChannelMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.network_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, None);
    }

    fn recv(&mut self, _ctx: &Context<Self::Msg>, msg: Self::Msg, _sender: Option<BasicActorRef>) {
        use log::*;
        let ts = self.start.elapsed().as_secs_f32();

        let (record_type, peer_id, record) = match msg {
            NetworkChannelMsg::PeerCreated(msg) => {
                (RecordType::PeerCreated, msg.peer.name().to_string(), Vec::new())
            }
            NetworkChannelMsg::PeerBootstrapped(msg) => {
                (RecordType::PeerBootstrapped, msg.peer.name().to_string(), msg.peer_id.into_bytes())
            }
            NetworkChannelMsg::PeerMessageReceived(msg) => {
                (RecordType::PeerReceivedMessage, msg.peer.name().to_string(), msg.message.as_bytes().unwrap_or_default())
            }
        };
        if let Err(err) = self.record_meta_storage.put_record_meta(ts, &RecordMeta {
            record_type,
            peer_id,
        }) {
            warn!("Failed to store meta for record: {}", err);
        }
        if let Err(err) = self.record_storage.put_record(ts, &record) {
            warn!("Failed to store record {}", err);
        }
    }
}