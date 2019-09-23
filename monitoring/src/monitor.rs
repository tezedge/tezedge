use log::*;
use networking::p2p::{
    network_channel::{NetworkChannelMsg, NetworkChannelTopic, PeerMessageReceived},
};
use riker::{
    system::SystemEvent, actor::*,
    system::Timer, actors::SystemMsg,
};
use std::{collections::HashMap, time::Duration};
use crate::{
    monitors::{PeerMonitor, BootstrapMonitor},
    handlers::handler_messages::PeerConnectionStatus,
    handlers::WebsocketHandlerMsg,
};
use crate::handlers::handler_messages::HandlerMessage;


#[derive(Clone, Debug)]
pub enum BroadcastSignal {
    Transfer,
    PeerUpdate(PeerConnectionStatus),
}

pub type MetricsManagerRef = ActorRef<MonitorMsg>;

#[actor(BroadcastSignal, NetworkChannelMsg, SystemEvent)]
pub struct Monitor {
    event_channel: ChannelRef<NetworkChannelMsg>,
    msg_channel: ActorRef<WebsocketHandlerMsg>,
    // Monitors
    peer_monitors: HashMap<usize, PeerMonitor>,
    bootstrap_monitor: BootstrapMonitor,
}

impl Monitor {
    fn name() -> &'static str {
        "metrics_manager"
    }

    fn new((event_channel, msg_channel): (ChannelRef<NetworkChannelMsg>, ActorRef<WebsocketHandlerMsg>)) -> Self {
        Self {
            event_channel,
            msg_channel,
            peer_monitors: HashMap::new(),
            bootstrap_monitor: BootstrapMonitor::new(),
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, event_channel: ChannelRef<NetworkChannelMsg>, msg_channel: ActorRef<WebsocketHandlerMsg>) -> Result<MetricsManagerRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, (event_channel, msg_channel)),
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

impl Actor for Monitor {
    type Msg = MonitorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        // Listen for all network events
        self.event_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, None);

        // Every second, send yourself a message to broadcast the monitoring to all connected clients
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

impl Receive<SystemEvent> for Monitor {
    type Msg = MonitorMsg;

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

impl Receive<BroadcastSignal> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: BroadcastSignal, _sender: Sender) {
        // TODO: Replace with channel implementation and broadcast it on that instead.
        let msg: HandlerMessage = match msg {
            BroadcastSignal::Transfer => self.peer_monitors.values_mut().collect(),
            BroadcastSignal::PeerUpdate(msg) => msg.into(),
        };
        self.msg_channel.tell(msg, None);
    }
}

impl Receive<NetworkChannelMsg> for Monitor {
    type Msg = MonitorMsg;

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
            NetworkChannelMsg::PeerBootstrapped(_msg) => {}
            NetworkChannelMsg::PeerMessageReceived(msg) => self.process_peer_message(msg),
        }
    }
}