// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezedge_actor_system::actors::*;

use rpc::RpcServiceEnvironmentRef;
use shell::shell_channel::{ShellChannelMsg, ShellChannelRef};
use shell::subscription::subscribe_to_shell_new_current_head;

#[actor(ShellChannelMsg)]
pub struct RpcNotificationCallbackActor {
    shell_channel: ShellChannelRef,
    rpc_env: RpcServiceEnvironmentRef,
}

impl RpcNotificationCallbackActor {
    pub fn actor(
        sys: &ActorSystem,
        shell_channel: ShellChannelRef,
        rpc_env: RpcServiceEnvironmentRef,
    ) -> Result<ActorRef<RpcNotificationCallbackActorMsg>, CreateError> {
        sys.actor_of_props::<RpcNotificationCallbackActor>(
            "rpc-notification-callback-actor",
            Props::new_args((shell_channel, rpc_env)),
        )
    }
}

impl ActorFactoryArgs<(ShellChannelRef, RpcServiceEnvironmentRef)>
    for RpcNotificationCallbackActor
{
    fn create_args((shell_channel, rpc_env): (ShellChannelRef, RpcServiceEnvironmentRef)) -> Self {
        RpcNotificationCallbackActor {
            shell_channel,
            rpc_env,
        }
    }
}

impl Actor for RpcNotificationCallbackActor {
    type Msg = RpcNotificationCallbackActorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_shell_new_current_head(&self.shell_channel, ctx.myself());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<ShellChannelMsg> for RpcNotificationCallbackActor {
    type Msg = RpcNotificationCallbackActorMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, msg: ShellChannelMsg, _: Sender) {
        if let ShellChannelMsg::NewCurrentHead(notification) = msg {
            // here we just update rpc thread-safe shared state
            rpc::handle_notify_rpc_server_msg(&self.rpc_env, notification)
        }
    }
}
