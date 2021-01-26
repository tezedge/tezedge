// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::HashMap, convert::Infallible, net::SocketAddr, sync::Arc};

use riker::actor::*;
use slog::{info, warn, Logger};
use tokio::runtime::Handle;
use tokio::sync::RwLock;
use warp::{ws::Message, Filter};

use crate::websocket::Clients;
use crate::websocket::{handler_messages::HandlerMessage, ws_server::ws_handler};

#[actor(HandlerMessage)]
pub struct WebsocketHandler {
    clients: Clients,
    tokio_executor: Handle,
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
        log: Logger,
    ) -> Result<WebsocketHandlerRef, CreateError> {
        info!(log, "Starting websocket server"; "address" => address);

        sys.actor_of_props::<WebsocketHandler>(
            Self::name(),
            Props::new_args((tokio_executor, address, log)),
        )
    }
}

impl ActorFactoryArgs<(Handle, SocketAddr, Logger)> for WebsocketHandler {
    fn create_args((tokio_executor, address, log): (Handle, SocketAddr, Logger)) -> Self {
        let clients: Clients = Arc::new(RwLock::new(HashMap::new()));

        let ws_route = warp::path::end()
            .and(warp::ws())
            .and(with_clients(clients.clone()))
            .and(with_log(log.clone()))
            .and_then(ws_handler)
            .with(warp::cors().allow_any_origin());

        println!("Starting ws task");
        tokio_executor.spawn(async move { warp::serve(ws_route).run(address).await });

        println!("Tokio ws task spawned");

        Self {
            clients,
            tokio_executor,
        }
    }
}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

fn with_log(log: Logger) -> impl Filter<Extract = (Logger,), Error = Infallible> + Clone {
    warp::any().map(move || log.clone())
}

impl Actor for WebsocketHandler {
    type Msg = WebsocketHandlerMsg;

    fn recv(&mut self, ctx: &Context<Self::Msg>, msg: Self::Msg, sender: Option<BasicActorRef>) {
        self.receive(ctx, msg, sender);
    }
}

impl Receive<HandlerMessage> for WebsocketHandler {
    type Msg = WebsocketHandlerMsg;

    fn receive(&mut self, ctx: &Context<Self::Msg>, msg: HandlerMessage, _sender: Sender) {
        let clients = self.clients.clone();
        let log = ctx.system.log();

        self.tokio_executor.spawn(async move {
            let clients = clients.read().await;
            if !clients.is_empty() {
                match serde_json::to_string(&msg) {
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
