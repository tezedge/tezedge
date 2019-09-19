use crate::ws_server::WsServer;
use ws::{WebSocket, Sender as WsSender};
use riker::{actor::*, system::Timer};
use std::thread::Builder;
use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelTopic};
use std::sync::{Arc, atomic::AtomicUsize};
use std::time::Duration;
use log::*;
use std::collections::HashMap;
use crate::metrics::{PeerMonitor, PeerMetrics};
use networking::p2p::network_channel::NetworkChannelMsg::PeerCreated;

#[derive(Clone, Debug)]
pub struct BroadcastSignal;

pub type MetricsManagerRef = ActorRef<MetricsManagerMsg>;

#[actor(BroadcastSignal, NetworkChannelMsg)]
pub struct MetricsManager {
    _socket_port: u16,
    event_channel: ChannelRef<NetworkChannelMsg>,
    broadcaster: WsSender,
    connected_clients: Arc<AtomicUsize>,
    peer_monitors: HashMap<String, PeerMonitor>,
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
                .expect("Unable to bind websocket server");
            socket.run().expect("Unable to run websocket server");
        }).expect("Failed to spawn websocket thread");
        info!("Starting websocket server at port: {}", socket_port);


        Self {
            _socket_port: socket_port,
            event_channel,
            broadcaster,
            connected_clients,
            peer_monitors: HashMap::new(),
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
        use serde_json::to_string;
        use std::sync::atomic::Ordering::Relaxed;

        if self.connected_clients.load(Relaxed) > 0 {
            // There are clients connected to the WebSocket
            let snapshot_data: HashMap<&String, PeerMetrics> = self.peer_monitors.iter_mut()
                .map(|(k, v)| {
                    (k, v.snapshot())
                }).collect();
            if let Ok(message) = to_string(&snapshot_data) {
                if let Err(err) = self.broadcaster.send(message) {
                    warn!("Failed broadcast message: {}", err);
                }
            } else {
                warn!("Failed to serialize snapshot data");
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for MetricsManager {
    type Msg = MetricsManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _sender: Sender) {
        use std::mem::size_of_val;
        match msg {
            NetworkChannelMsg::PeerCreated(msg) => {
                // TODO: Add field to message with peer name / identifier
                let peer_name = msg.peer.name().to_owned();
                let monitor = PeerMonitor::new(peer_name.clone());
                self.peer_monitors.insert(peer_name, monitor);
            }
            NetworkChannelMsg::PeerDisconnected(msg) => {
                // TODO: Add field identifying disconnected peer
            }
            NetworkChannelMsg::PeerBootstrapped(msg) => {}
            NetworkChannelMsg::PeerMessageReceived(msg) => {
                if let Some(monitor) = self.peer_monitors.get_mut(msg.peer.name()) {
                    // TODO: Add proper sizes
                    monitor.incoming_bytes(size_of_val(&msg.message));
                } else {
                    warn!("Missing monitor for peer: {}", msg.peer.name());
                }
            }
        }
    }
}