use log::*;
use networking::p2p::{
    network_channel::{NetworkChannelMsg, NetworkChannelTopic, PeerMessageReceived, NetworkChannelRef},
};
use shell::shell_channel::{ShellChannelRef, ShellChannelMsg, ShellChannelTopic};
use riker::{
    system::SystemEvent, actor::*,
    system::Timer, actors::SystemMsg,
};
use std::{collections::HashMap, time::Duration};
use crate::{
    monitors::*,
    handlers::handler_messages::PeerConnectionStatus,
    handlers::WebsocketHandlerMsg,
};
use crate::handlers::handler_messages::HandlerMessage;


#[derive(Clone, Debug)]
pub enum BroadcastSignal {
    PublishPeerStatistics,
    PublishBlocksStatistics,
    PeerUpdate(PeerConnectionStatus),
}

pub type MonitorRef = ActorRef<MonitorMsg>;

#[actor(BroadcastSignal, NetworkChannelMsg, SystemEvent, ShellChannelMsg)]
pub struct Monitor {
    event_channel: NetworkChannelRef,
    shell_channel: ShellChannelRef,
    msg_channel: ActorRef<WebsocketHandlerMsg>,
    // Monitors
    peer_monitors: HashMap<ActorUri, PeerMonitor>,
    bootstrap_monitor: BootstrapMonitor,
    blocks_monitor: BlocksMonitor,
}

impl Monitor {
    fn name() -> &'static str {
        "monitor-manager"
    }

    fn new((event_channel, msg_channel, shell_channel): (NetworkChannelRef, ActorRef<WebsocketHandlerMsg>, ShellChannelRef)) -> Self {
        Self {
            event_channel,
            shell_channel,
            msg_channel,
            peer_monitors: HashMap::new(),
            bootstrap_monitor: BootstrapMonitor::new(),
            blocks_monitor: BlocksMonitor::new(),
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, event_channel: NetworkChannelRef, msg_channel: ActorRef<WebsocketHandlerMsg>, shell_channel: ShellChannelRef) -> Result<MonitorRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, (event_channel, msg_channel, shell_channel)),
            Self::name(),
        )
    }

    pub fn process_peer_message(&mut self, msg: PeerMessageReceived) {
        use std::mem::size_of_val;
        use networking::p2p::encoding::peer::PeerMessage;

        // TODO: Add real message processing

        for message in &msg.message.messages {
            match message {
                PeerMessage::CurrentBranch(msg) => {
                    self.blocks_monitor.current_level = Some(msg.current_branch.current_head.level);
                }
                _ => (),
            }
        }

        if let Some(monitor) = self.peer_monitors.get_mut(msg.peer.uri()) {
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
        self.shell_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, None);

        // Every second, send yourself a message to broadcast the monitoring to all connected clients
        ctx.schedule(Duration::from_secs(1),
                     Duration::from_secs(1),
                     ctx.myself(), None,
                     BroadcastSignal::PublishPeerStatistics);
        ctx.schedule(Duration::from_secs_f32(1.5),
                     Duration::from_secs(1),
                     ctx.myself(), None,
                     BroadcastSignal::PublishBlocksStatistics);
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
            if let Some(monitor) = self.peer_monitors.remove(evt.actor.uri()) {
                let name = if let Some(addr) = monitor.addr {
                    addr.to_string()
                } else {
                    evt.actor.name().to_string()
                };
                ctx.myself.tell(
                    BroadcastSignal::PeerUpdate(PeerConnectionStatus::disconnected(name)),
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
        match msg {
            BroadcastSignal::PublishPeerStatistics => {
                let peer_stats: HandlerMessage = self.peer_monitors.values_mut().collect();
                self.msg_channel.tell(peer_stats, None);
            }
            BroadcastSignal::PublishBlocksStatistics => {
                let bootstrap_stats: HandlerMessage = self.bootstrap_monitor.snapshot().into();
                self.msg_channel.tell(bootstrap_stats, None);
            }
            BroadcastSignal::PeerUpdate(msg) => {
                let msg: HandlerMessage = msg.into();
                self.msg_channel.tell(msg, None)
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _sender: Sender) {
        match msg {
            NetworkChannelMsg::PeerCreated(msg) => {
                // TODO: Add field to message with peer name / identifier
                let identifier = msg.peer.uri();
                let mut monitor = PeerMonitor::new(identifier.clone());
                monitor.addr = Some(msg.address);
                monitor.public_key = Some(msg.public_key.clone());
                if let Some(monitor) = self.peer_monitors.insert(msg.peer.uri().clone(), monitor) {
                    warn!("Duplicate monitor found for peer: {}", monitor.identifier.to_string());
                }
                ctx.myself.tell(
                    BroadcastSignal::PeerUpdate(PeerConnectionStatus::connected(msg.address.to_string())),
                    None,
                );
            }
            NetworkChannelMsg::PeerBootstrapped(_msg) => {}
            NetworkChannelMsg::PeerMessageReceived(msg) => self.process_peer_message(msg),
        }
    }
}

impl Receive<ShellChannelMsg> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        use std::cmp::max;

        match msg {
            ShellChannelMsg::BlockReceived(msg) => {
                if let Some(level) = self.blocks_monitor.current_level {
                    self.bootstrap_monitor.level = max(level, msg.level) as usize;
                } else {
                    self.bootstrap_monitor.level = msg.level as usize
                }
                // Increase block header count
                self.bootstrap_monitor.increase_block_count();

                // TODO: Add block statistics
            }
            ShellChannelMsg::BlockApplied(_msg) => {}
            ShellChannelMsg::AllBlockOperationsReceived(_msg) => {}
        }
    }
}