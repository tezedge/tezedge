use networking::p2p::network_channel::{NetworkChannelMsg, NetworkChannelTopic, NetworkChannelRef};
use shell::shell_channel::{ShellChannelRef, ShellChannelMsg, ShellChannelTopic};
use riker::{
    actor::*,
};
use crate::{
    helpers::*,
    server::control_msg::*,
};
use slog::warn;

pub type RpcServerRef = ActorRef<RpcServerMsg>;

#[actor(NetworkChannelMsg, ShellChannelMsg, GetCurrentHead)]
pub struct RpcServer {
    network_channel: NetworkChannelRef,
    shell_channel: ShellChannelRef,
    // Stats
    current_head: Option<CurrentHead>,
}

impl RpcServer {
    fn name() -> &'static str { "rpc-server" }

    fn new((network_channel, shell_channel): (NetworkChannelRef, ShellChannelRef)) -> Self {
        Self {
            network_channel,
            shell_channel,
            current_head: None,
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, network_channel: NetworkChannelRef, shell_channel: ShellChannelRef) -> Result<RpcServerRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, (network_channel, shell_channel)),
            Self::name(),
        )
    }
}

impl Actor for RpcServer {
    type Msg = RpcServerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        self.network_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: NetworkChannelTopic::NetworkEvents.into(),
        }, ctx.myself().into());

        self.shell_channel.tell(Subscribe {
            actor: Box::new(ctx.myself()),
            topic: ShellChannelTopic::ShellEvents.into(),
        }, ctx.myself().into());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<NetworkChannelMsg> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, _msg: NetworkChannelMsg, _sender: Sender) {
        unimplemented!()
    }
}

impl Receive<ShellChannelMsg> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: ShellChannelMsg, _sender: Sender) {
        match msg {
            ShellChannelMsg::BlockApplied(data) => {
                if let Some(ref current_head) = self.current_head {
                    if current_head.level() < data.level {
                        self.current_head = Some(CurrentHead::new(data.level, data.hash.clone()));
                    }
                } else {
                    self.current_head = Some(CurrentHead::new(data.level, data.hash.clone()));
                }
            }
            _ => ( /* Not yet implemented, do nothing */),
        }
    }
}

impl Receive<GetCurrentHead> for RpcServer {
    type Msg = RpcServerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: GetCurrentHead, sender: Sender) {
        if let GetCurrentHead::Request = msg {
            if let Some(sender) = sender {
                let me: Option<BasicActorRef> = ctx.myself().into();
                if sender.try_tell(GetCurrentHead::Response(self.current_head.clone()), me).is_err() {
                    warn!(ctx.system.log(), "Failed to send response for GetCurrentHead");
                }
            }
        }
    }
}