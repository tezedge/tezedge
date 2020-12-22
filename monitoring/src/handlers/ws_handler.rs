// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    net::SocketAddr,
    sync::{atomic::AtomicUsize, Arc},
    thread::Builder,
};

use riker::actor::*;
use slog::{info, warn, Logger};
use ws::{Sender as WsSender, WebSocket};

use crate::handlers::{handler_messages::HandlerMessage, ws_server::WsServer};

#[actor(HandlerMessage)]
pub struct WebsocketHandler {
    broadcaster: WsSender,
    connected_clients: Arc<AtomicUsize>,
}

pub type WebsocketHandlerRef = ActorRef<WebsocketHandlerMsg>;

impl WebsocketHandler {
    pub fn name() -> &'static str {
        "websocket_handler"
    }

    pub fn actor(
        sys: &impl ActorRefFactory,
        address: SocketAddr,
        log: Logger,
    ) -> Result<WebsocketHandlerRef, CreateError> {
        info!(log, "Starting websocket server"; "address" => address);

        sys.actor_of_props::<WebsocketHandler>(Self::name(), Props::new_args(address))
    }
}

impl ActorFactoryArgs<SocketAddr> for WebsocketHandler {
    fn create_args(address: SocketAddr) -> Self {
        let connected_clients = Arc::new(AtomicUsize::new(0));
        let ws_server = WebSocket::new(WsServer::new(connected_clients.clone()))
            .expect("Unable to create websocket server");
        let broadcaster = ws_server.broadcaster();

        Builder::new()
            .name("ws_handler".to_string())
            .spawn(move || {
                let socket = ws_server
                    .bind(address)
                    .expect("Unable to bind websocket server");
                socket.run().expect("Websocket failed unexpectedly");
            })
            .expect("Failed to spawn websocket thread");

        Self {
            broadcaster,
            connected_clients,
        }
    }
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
        use std::sync::atomic::Ordering;

        if self.connected_clients.load(Ordering::Acquire) > 0 {
            match serde_json::to_string(&msg) {
                Ok(serialized) => {
                    if let Err(err) = self.broadcaster.send(serialized) {
                        warn!(ctx.system.log(), "Failed to broadcast message"; "message" => msg, "reason" => format!("{:?}", err));
                    }
                }
                Err(err) => {
                    warn!(ctx.system.log(), "Failed to serialize message"; "message" => msg, "reason" => format!("{:?}", err))
                }
            }
        }
    }
}
