// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use tezedge_actor_system::actors::*;

use networking::network_channel::{NetworkChannelMsg, NetworkChannelRef};
use rpc::RpcServiceEnvironmentRef;
use shell::subscription::subscribe_to_network_events;

#[actor(NetworkChannelMsg)]
pub struct RpcNotificationCallbackActor {
    network_channel: NetworkChannelRef,
    rpc_env: RpcServiceEnvironmentRef,
}

impl RpcNotificationCallbackActor {
    pub fn actor(
        sys: &ActorSystem,
        network_channel: NetworkChannelRef,
        rpc_env: RpcServiceEnvironmentRef,
    ) -> Result<ActorRef<RpcNotificationCallbackActorMsg>, CreateError> {
        sys.actor_of_props::<RpcNotificationCallbackActor>(
            "rpc-notification-callback-actor",
            Props::new_args((network_channel, rpc_env)),
        )
    }
}

impl ActorFactoryArgs<(NetworkChannelRef, RpcServiceEnvironmentRef)>
    for RpcNotificationCallbackActor
{
    fn create_args(
        (network_channel, rpc_env): (NetworkChannelRef, RpcServiceEnvironmentRef),
    ) -> Self {
        RpcNotificationCallbackActor {
            network_channel,
            rpc_env,
        }
    }
}

impl Actor for RpcNotificationCallbackActor {
    type Msg = RpcNotificationCallbackActorMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        subscribe_to_network_events(&self.network_channel, ctx.myself());
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Sender) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<NetworkChannelMsg> for RpcNotificationCallbackActor {
    type Msg = RpcNotificationCallbackActorMsg;

    fn receive(&mut self, _: &Context<Self::Msg>, msg: NetworkChannelMsg, _: Sender) {
        if let NetworkChannelMsg::NewCurrentHead(notification) = msg {
            // here we just update rpc thread-safe shared state
            rpc::handle_notify_rpc_server_msg(&self.rpc_env, notification)
        }
    }
}
