// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::Duration;
use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use rpc::RpcServiceEnvironmentRef;
use slog::{info, warn, Logger};
use tezedge_actor_system::{actor::*, system::Timer};
use tokio::runtime::Handle;
use tokio::sync::RwLock;
use warp::ws::Message;

use crate::websocket::ws_messages::WebsocketMessageWrapper;
use crate::websocket::ws_server::run_websocket;
use crate::websocket::Clients;

use super::RpcClients;

/// How often to print stats in logs
const LOG_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Clone, Debug)]
pub struct LogStats;

#[actor(WebsocketMessageWrapper, LogStats)]
pub struct WebsocketHandler {
    clients: Clients,
    tokio_executor: Handle,
    /// Count of received messages from the last log
    actor_received_messages_count: usize,
}

pub type WebsocketHandlerRef = ActorRef<WebsocketHandlerMsg>;

impl WebsocketHandler {
    pub fn name() -> &'static str {
        "websocket_handler"
    }

    pub fn actor(
        sys: &impl ActorRefFactory,
        tokio_executor: Handle,
        address: SocketAddr,
        max_number_of_websocket_connections: u16,
        log: Logger,
        rpc_env: RpcServiceEnvironmentRef,
    ) -> Result<WebsocketHandlerRef, CreateError> {
        info!(log, "Starting monitoring websocket server";
                   "address" => address,
                   "max_number_of_websocket_connections" => max_number_of_websocket_connections);

        sys.actor_of_props::<WebsocketHandler>(
            Self::name(),
            Props::new_args((
                tokio_executor,
                address,
                max_number_of_websocket_connections,
                log,
                rpc_env,
            )),
        )
    }

    fn get_and_clear_actor_received_messages_count(&mut self) -> usize {
        std::mem::replace(&mut self.actor_received_messages_count, 0)
    }
}

impl ActorFactoryArgs<(Handle, SocketAddr, u16, Logger, RpcServiceEnvironmentRef)>
    for WebsocketHandler
{
    fn create_args(
        (tokio_executor, address, max_number_of_websocket_connections, log, rpc_env): (
            Handle,
            SocketAddr,
            u16,
            Logger,
            RpcServiceEnvironmentRef,
        ),
    ) -> Self {
        let monitoring_clients: Clients = Arc::new(RwLock::new(HashMap::new()));
        let rpc_clients: RpcClients = Arc::new(RwLock::new(HashMap::new()));

        {
            let monitoring_clients = monitoring_clients.clone();
            tokio_executor.spawn(async move {
                info!(log, "Starting websocket server"; "address" => format!("{}", &address));
                run_websocket(
                    address,
                    max_number_of_websocket_connections,
                    monitoring_clients,
                    rpc_clients,
                    rpc_env,
                    log,
                )
                .await
            });
        }

        Self {
            clients: monitoring_clients,
            tokio_executor,
            actor_received_messages_count: 0,
        }
    }
}

impl Actor for WebsocketHandler {
    type Msg = WebsocketHandlerMsg;

    fn pre_start(&mut self, ctx: &Context<Self::Msg>) {
        ctx.schedule::<Self::Msg, _>(
            LOG_INTERVAL / 2,
            LOG_INTERVAL,
            ctx.myself(),
            None,
            LogStats.into(),
        );
    }

    fn post_start(&mut self, ctx: &Context<Self::Msg>) {
        info!(ctx.system.log(), "Monitoring websocket handler started");
    }

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.actor_received_messages_count += 1;
        self.receive(ctx, msg, sender);
    }
}

impl Receive<WebsocketMessageWrapper> for WebsocketHandler {
    type Msg = WebsocketHandlerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: WebsocketMessageWrapper, _sender: Sender) {
        let clients = self.clients.clone();
        let log = ctx.system.log();

        self.tokio_executor.spawn(async move {
            let clients = clients.read().await;
            if !clients.is_empty() {
                match serde_json::to_string(&msg.messages) {
                    Ok(serialized) => {
                        clients.iter().for_each(|(_, client_sender)| {
                            if let Some(sender) = &client_sender {
                                let _ = sender.send(Ok(Message::text(serialized.clone())));
                            }
                        });
                    }
                    Err(err) => {
                        warn!(log, "Failed to serialize message"; "message" => msg, "reason" => format!("{:?}", err))
                    }
                }
            }
        });
    }
}

impl Receive<LogStats> for WebsocketHandler {
    type Msg = WebsocketHandlerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, _: LogStats, _: Sender) {
        let actor_received_messages_count = self.get_and_clear_actor_received_messages_count();
        let clients = self.clients.clone();
        let log = ctx.system.log();

        self.tokio_executor.spawn(async move {
            let clients = clients.read().await;
            info!(log, "Monitoring websocket handler info";
                   "actor_received_messages_count" => actor_received_messages_count,
                   "clients" => clients.len(),
            );
        });
    }
}
