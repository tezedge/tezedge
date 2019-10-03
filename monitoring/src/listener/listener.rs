use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelRef, NetworkChannelTopic};
use riker::actor::*;
use std::fs::File;
use crate::listener::records::Record;
use std::io::Write;
use log::*;

type NetworkListenerRef = ActorRef<NetworkChannelMsg>;

pub struct NetworkListener {
    storage: File,
    network_channel: NetworkChannelRef,
}

impl NetworkListener {
    fn name() -> &'static str { "network-listener" }

    fn new((storage_path, network_channel): (&'static str, NetworkChannelRef)) -> Self {
        Self {
            storage: File::open(storage_path).expect("Failed to open the file"),
            network_channel,
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, storage_path: &'static str, network_channel: NetworkChannelRef) -> Result<NetworkListenerRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, (storage_path, network_channel)),
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
        let record: Record = msg.into();
        if let Ok(record) = bincode::serialize(&record) {
            if let Err(err) = self.storage.write_all(&record) {
                warn!("Failed to store incoming message: {}", err);
            }
            if let Err(err) = writeln!(self.storage) {
                warn!("Failed to write newline after message: {}", err);
            }
        }
    }
}