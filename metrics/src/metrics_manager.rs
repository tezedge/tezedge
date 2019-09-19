use crate::ws_server::WsServer;
use ws::{WebSocket, Sender as WsSender};
use riker::{actor::*, system::Timer};
use std::thread::Builder;
use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelTopic};
use std::sync::{Arc, atomic::AtomicUsize};
use std::time::Duration;
use log::*;

#[derive(Clone, Debug)]
pub struct BroadcastSignal;

pub type MetricsManagerRef = ActorRef<MetricsManagerMsg>;

#[actor(BroadcastSignal, NetworkChannelMsg)]
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
            let socket = ws_server.bind(("localhost", socket_port))
                .expect("Unable to start websocket server");
            socket.run();
        }).expect("Failed to spawn websocket thread");
        info!("Starting websocket server at port: {}", socket_port);


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
    type Msg = MetricsManagerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // Listen for all network events
        self.event_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, None);

        // Every second, send yourself a message to broadcast the metrics to all connected clients
        ctx.schedule(Duration::from_secs(1),
                     Duration::from_secs(1),
                     ctx.myself(), None,
                     BroadcastSignal);
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<BroadcastSignal> for MetricsManager {
    type Msg = MetricsManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: BroadcastSignal, _sender: Sender) {
        use std::sync::atomic::Ordering::Relaxed;
        if self.connected_clients.load(Relaxed) > 0 {
            // There are clients connected to the WebSocket
            // TODO: Send metrics data
            if let Err(err) = self.broadcaster.send("{ health: \"ok\" }") {
                warn!("Failed broadcast message: {}", err);
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for MetricsManager {
    type Msg = MetricsManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: NetworkChannelMsg, _sender: Sender) {}
}