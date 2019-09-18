use crate::ws_server::{WsClient, WsServer};
use ws::{WebSocket, Sender as WsSender};
use riker::actor::*;
use std::thread::Builder;
use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelTopic};
use std::sync::{Arc, atomic::AtomicUsize};

pub type MetricsManagerRef = ActorRef<NetworkChannelMsg>;

pub struct MetricsManager {
    socket_port: u16,
    event_channel: ChannelRef<NetworkChannelMsg>,
    broadcaster: WsSender,
    connected_clients: Arc<AtomicUsize>,
}

impl MetricsManager {
    fn name() -> &'static str {
        "metrics_manager"
    }

    fn new((event_channel, socket_port): (ChannelRef<NetworkChannelMsg>, u16)) -> Self {
        let connected_clients = Arc::new(AtomicUsize::new(0));
        let ws_server = WebSocket::new(WsServer::new(connected_clients.clone()))
            .expect("Unable to create websocket server");
        let broadcaster = ws_server.broadcaster();
        Builder::new().name("ws_metrics_server".to_owned()).spawn(move || {
            ws_server.run()
        });


        Self {
            socket_port,
            event_channel,
            broadcaster,
            connected_clients,
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, event_channel: ChannelRef<NetworkChannelMsg>, socket_port: u16) -> Result<MetricsManagerRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, (event_channel, socket_port)),
            Self::name(),
        )
    }
}

impl Actor for MetricsManager {
    type Msg = NetworkChannelMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // Listen for all network events
        self.event_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, None);
    }

    fn recv(&mut self, _ctx: &Context<Self::Msg>, _msg: Self::Msg, _sender: Option<BasicActorRef>) {
        unimplemented!()
    }
}