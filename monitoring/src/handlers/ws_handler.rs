use ws::{WebSocket, Sender as WsSender};
use std::{
    sync::{Arc, atomic::AtomicUsize},
    net::SocketAddr,
    thread::Builder,
};
use log::*;
use riker::actor::*;
use crate::handlers::{
    ws_server::WsServer,
    handler_messages::HandlerMessage,
};

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

    pub fn new(address: SocketAddr) -> Self {
        let connected_clients = Arc::new(AtomicUsize::new(0));
        let ws_server = WebSocket::new(WsServer::new(connected_clients.clone()))
            .expect("Unable to create websocket server");
        let broadcaster = ws_server.broadcaster();

        Builder::new().name("ws_handler".to_string()).spawn(move || {
            let socket = ws_server.bind(address)
                .expect("Unable to bind websocket server");
            socket.run().expect("Websocket failed unexpectedly");
        }).expect("Failed to spawn websocket thread");
        info!("Starting websocket server at address: {}", address);

        Self {
            broadcaster,
            connected_clients,
        }
    }

    pub fn actor(sys: &impl ActorRefFactory, address: SocketAddr) -> Result<WebsocketHandlerRef, CreateError> {
        sys.actor_of(
            Props::new_args(Self::new, address),
            Self::name(),
        )
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

    fn receive(&mut self, _ctx: &Context<Self::Msg>, msg: HandlerMessage, _sender: Sender) {
        use std::sync::atomic::Ordering::Relaxed;

        if self.connected_clients.load(Relaxed) > 0 {
            match serde_json::to_string(&msg) {
                Ok(serialized) => if let Err(err) = self.broadcaster.send(serialized) {
                    warn!("Failed to broadcast message: {}", err);
                }
                Err(err) => warn!("Failed to serialize message '{:?}: {}'", msg, err)
            }
        }
    }
}