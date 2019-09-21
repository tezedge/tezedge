use log::*;
use ws::{WebSocket, Sender as WsSender};
use networking::p2p::{
    network_channel::{NetworkChannelMsg, NetworkChannelTopic, PeerMessageReceived},
    encoding::peer::PeerMessage,
};
use riker::{
    system::SystemEvent, actor::*,
    system::Timer, actors::SystemMsg,
};
use std::{
    collections::HashMap, time::Duration,
    sync::{Arc, atomic::AtomicUsize},
    thread::Builder,
};
use crate::{
    monitors::{PeerMonitor, BootstrapMonitor}, ws_server::WsServer,
    messages::PeerConnectionStatus,
};


#[derive(Clone, Debug)]
pub enum BroadcastSignal {
    Transfer,
    PeerUpdate(PeerConnectionStatus),
}

pub type MetricsManagerRef = ActorRef<MetricsManagerMsg>;

#[actor(BroadcastSignal, NetworkChannelMsg, SystemEvent)]
pub struct MetricsManager {
    _socket_port: u16,
    event_channel: ChannelRef<NetworkChannelMsg>,
    broadcaster: WsSender,
    connected_clients: Arc<AtomicUsize>,
    // Monitors
    peer_monitors: HashMap<usize, PeerMonitor>,
    bootstrap_monitor: BootstrapMonitor,
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
        // let sender = ws_server

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
            bootstrap_monitor: BootstrapMonitor::new(),
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, event_channel: ChannelRef<NetworkChannelMsg>, socket_port: u16) -> Result<MetricsManagerRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, (event_channel, socket_port)),
            Self::name(),
        )
    }

    pub fn process_peer_message(&mut self, msg: PeerMessageReceived) {
        use std::mem::size_of_val;

        // TODO: Add real message processing
        // TODO: Add general statistics processing (ETA, blocks)

        for message in &msg.message.messages {
            match message {
                _ => info!("Not implemented monitoring for: {:?}", msg),
            }
        }

        if let Some(monitor) = self.peer_monitors.get_mut(&msg.peer.uri().uid) {
            // TODO: Add proper sizes
            monitor.incoming_bytes(size_of_val(&msg.message));
        } else {
            warn!("Missing monitor for peer: {}", msg.peer.name());
        }
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
                     BroadcastSignal::Transfer);
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }

    fn sys_recv(&mut self, ctx: &Context<Self::Msg>, msg: SystemMsg, sender: Option<BasicActorRef>) {
        if let SystemMsg::Event(evt) = msg {
            self.receive(ctx, evt, sender);
        }
    }
}

impl Receive<SystemEvent> for MetricsManager {
    type Msg = MetricsManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: SystemEvent, _sender: Sender) {
        if let SystemEvent::ActorTerminated(evt) = msg {
            if let Some(_monitor) = self.peer_monitors.remove(&evt.actor.uri().uid) {
                ctx.myself.tell(
                    BroadcastSignal::PeerUpdate(PeerConnectionStatus::disconnected(evt.actor.name().to_owned())),
                    None,
                );
            } else {
                warn!("Monitor for actor {}, was never set up.", evt.actor.name());
            }
        }
    }
}

impl Receive<BroadcastSignal> for MetricsManager {
    type Msg = MetricsManagerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: BroadcastSignal, _sender: Sender) {
        use crate::messages::Message;
        use std::sync::atomic::Ordering::Relaxed;

        if self.connected_clients.load(Relaxed) > 0 {
            let conn_msg: Message = match msg {
                BroadcastSignal::Transfer => self.peer_monitors.values_mut().collect(),
                BroadcastSignal::PeerUpdate(msg) => msg.into(),
            };

            match serde_json::to_string(&conn_msg) {
                Ok(serialized_msg) => if let Err(err) = self.broadcaster.send(serialized_msg) {
                    warn!("Failed to broadcast message: {}", err);
                }
                Err(err) => warn!("Failed to serialize message '{:?}': {}", conn_msg, err)
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for MetricsManager {
    type Msg = MetricsManagerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _sender: Sender) {
        match msg {
            NetworkChannelMsg::PeerCreated(msg) => {
                // TODO: Add field to message with peer name / identifier
                let identifier = msg.peer.uri();
                let monitor = PeerMonitor::new(identifier.clone());
                self.peer_monitors.insert(identifier.uid, monitor);
                ctx.myself.tell(
                    BroadcastSignal::PeerUpdate(PeerConnectionStatus::connected(format!("{}", identifier.uid))),
                    None,
                );
            }
            NetworkChannelMsg::PeerBootstrapped(msg) => {}
            NetworkChannelMsg::PeerMessageReceived(msg) => self.process_peer_message(msg),
        }
    }
}