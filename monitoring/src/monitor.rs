// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::time::Duration;

use riker::{actor::*, actors::SystemMsg, system::SystemEvent, system::Timer};
use slog::{warn, Logger};

use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelRef, PeerMessageReceived};
use shell::shell_channel::{ShellChannelMsg, ShellChannelRef};
use shell::subscription::{
    subscribe_to_actor_terminated, subscribe_to_network_events, subscribe_to_shell_events,
};
use tezos_messages::p2p::binary_message::BinaryMessage;

use crate::handlers::handler_messages::HandlerMessage;
use crate::{
    handlers::handler_messages::PeerConnectionStatus, handlers::WebsocketHandlerMsg, monitors::*,
};

#[derive(Clone, Debug)]
pub enum BroadcastSignal {
    PublishPeerStatistics,
    PublishBlocksStatistics,
    PeerUpdate(PeerConnectionStatus),
}

pub type MonitorRef = ActorRef<MonitorMsg>;

#[actor(BroadcastSignal, NetworkChannelMsg, SystemEvent, ShellChannelMsg)]
pub struct Monitor {
    network_channel: NetworkChannelRef,
    shell_channel: ShellChannelRef,
    msg_channel: ActorRef<WebsocketHandlerMsg>,
    // Monitors
    peer_monitors: HashMap<ActorUri, PeerMonitor>,
    bootstrap_monitor: BootstrapMonitor,
    blocks_monitor: BlocksMonitor,
    block_application_monitor: ApplicationMonitor,
    chain_monitor: ChainMonitor,
}

impl Monitor {
    fn name() -> &'static str {
        "monitor-manager"
    }

    pub fn actor(
        sys: &impl ActorRefFactory,
        event_channel: NetworkChannelRef,
        msg_channel: ActorRef<WebsocketHandlerMsg>,
        shell_channel: ShellChannelRef,
    ) -> Result<MonitorRef, CreateError> {
        sys.actor_of_props::<Monitor>(
            Self::name(),
            Props::new_args((event_channel, msg_channel, shell_channel)),
        )
    }

    fn process_peer_message(&mut self, msg: PeerMessageReceived, log: &Logger) {
        use std::mem::size_of_val;
        use tezos_messages::p2p::encoding::peer::PeerMessage;

        for message in msg.message.messages() {
            match message {
                PeerMessage::CurrentBranch(msg) => {
                    if msg.current_branch().current_head().level() > 0 {
                        self.bootstrap_monitor
                            .set_level(msg.current_branch().current_head().level() as usize);
                    }
                }
                _ => (),
            }
        }

        if let Some(monitor) = self.peer_monitors.get_mut(msg.peer.uri()) {
            if monitor.public_key.is_some() {
                // TODO: TE-190 - reimplement correctly, now not all messages are counted in (Ack, Metadata, ConnectionMessage is not involved)
                let size = if let Ok(msg) = msg.message.as_bytes() {
                    msg.len()
                } else {
                    size_of_val(&msg.message)
                };
                monitor.incoming_bytes(size);
            }
        } else {
            warn!(log, "Missing monitor for peer"; "peer" => msg.peer.name());
        }
    }
}

impl
    ActorFactoryArgs<(
        NetworkChannelRef,
        ActorRef<WebsocketHandlerMsg>,
        ShellChannelRef,
    )> for Monitor
{
    fn create_args(
        (event_channel, msg_channel, shell_channel): (
            NetworkChannelRef,
            ActorRef<WebsocketHandlerMsg>,
            ShellChannelRef,
        ),
    ) -> Self {
        Self {
            network_channel: event_channel,
            shell_channel,
            msg_channel,
            peer_monitors: HashMap::new(),
            bootstrap_monitor: BootstrapMonitor::default(),
            blocks_monitor: BlocksMonitor::new(4096, 0),
            block_application_monitor: ApplicationMonitor::new(),
            chain_monitor: ChainMonitor::new(),
        }
    }
}

impl Actor for Monitor {
    type Msg = MonitorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_actor_terminated(ctx.system.sys_events(), ctx.myself());
        subscribe_to_shell_events(&self.shell_channel, ctx.myself());
        subscribe_to_network_events(&self.network_channel, ctx.myself());

        // Every second, send yourself a message to broadcast the monitoring to all connected clients
        ctx.schedule(
            Duration::from_secs(1),
            Duration::from_secs(1),
            ctx.myself(),
            None,
            BroadcastSignal::PublishPeerStatistics,
        );
        ctx.schedule(
            Duration::from_secs_f32(1.5),
            Duration::from_secs(1),
            ctx.myself(),
            None,
            BroadcastSignal::PublishBlocksStatistics,
        );
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }

    fn sys_recv(
        &mut self,
        ctx: &Context<Self::Msg>,
        msg: SystemMsg,
        sender: Option<BasicActorRef>,
    ) {
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
                warn!(ctx.system.log(), "Monitor for actor, was never set up."; "actor" => evt.actor.name());
            }
        }
    }
}

impl Receive<BroadcastSignal> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: BroadcastSignal, _sender: Sender) {
        match msg {
            BroadcastSignal::PublishPeerStatistics => {
                let peer_stats: HandlerMessage = self.peer_monitors.values_mut().collect();
                self.msg_channel.tell(peer_stats, ctx.myself().into());
            }
            BroadcastSignal::PublishBlocksStatistics => {
                let bootstrap_stats: HandlerMessage = self.bootstrap_monitor.snapshot().into();
                self.msg_channel.tell(bootstrap_stats, ctx.myself().into());

                let payload = self.blocks_monitor.snapshot();
                self.msg_channel
                    .tell(HandlerMessage::BlockStatus { payload }, ctx.myself().into());

                let payload = self.block_application_monitor.snapshot();
                self.msg_channel.tell(
                    HandlerMessage::BlockApplicationStatus { payload },
                    ctx.myself().into(),
                );

                let payload = self.chain_monitor.snapshot();
                self.msg_channel
                    .tell(HandlerMessage::ChainStatus { payload }, ctx.myself().into());
            }
            BroadcastSignal::PeerUpdate(msg) => {
                let msg: HandlerMessage = msg.into();
                self.msg_channel.tell(msg, ctx.myself().into())
            }
        }
    }
}

impl Receive<NetworkChannelMsg> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: NetworkChannelMsg, _sender: Sender) {
        match msg {
            NetworkChannelMsg::PeerCreated(msg) => {
                let identifier = msg.peer.uri();
                let mut monitor = PeerMonitor::new(identifier.clone());
                monitor.addr = Some(msg.address);
                if let Some(monitor) = self.peer_monitors.insert(msg.peer.uri().clone(), monitor) {
                    warn!(ctx.system.log(), "Duplicate monitor found for peer"; "peer" => monitor.identifier.to_string());
                }
                ctx.myself.tell(
                    BroadcastSignal::PeerUpdate(PeerConnectionStatus::connected(
                        msg.address.to_string(),
                    )),
                    None,
                );
            }
            NetworkChannelMsg::PeerBootstrapped(peer_id, _, _) => {
                if let Some(monitor) = self.peer_monitors.get_mut(peer_id.peer_ref.uri()) {
                    monitor.public_key = Some(peer_id.peer_id_marker.clone());
                }
            }
            NetworkChannelMsg::PeerMessageReceived(msg) => {
                self.process_peer_message(msg, &ctx.system.log())
            }
            _ => (),
        }
    }
}

impl Receive<ShellChannelMsg> for Monitor {
    type Msg = MonitorMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match msg {
            ShellChannelMsg::BlockReceived(msg) => {
                // Update current max block count
                self.bootstrap_monitor.set_level(msg.level as usize);

                // Start tracking it in the blocks monitor
                self.blocks_monitor.accept_block();
                self.bootstrap_monitor.increase_headers_count();

                // update stats for block header
                self.chain_monitor.process_block_header(msg.level as usize);
            }
            ShellChannelMsg::NewCurrentHead(head, ..) => {
                // update stats for block applications
                self.chain_monitor
                    .process_block_application(*head.level() as usize);

                self.blocks_monitor.block_was_applied_by_protocol();
                self.block_application_monitor.block_was_applied(head);
            }
            ShellChannelMsg::AllBlockOperationsReceived(msg) => {
                self.bootstrap_monitor.increase_block_count();
                self.blocks_monitor.block_finished_downloading_operations();

                // update stats for block operations
                self.chain_monitor
                    .process_block_operations(msg.level as usize);
            }
            _ => (),
        }
    }
}
