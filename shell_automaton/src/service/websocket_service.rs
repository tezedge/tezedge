use futures_util::{
    stream::{SplitSink, SplitStream, StreamExt},
    SinkExt,
};
use serde::{Deserialize, Serialize};
use slog::{crit, error, info, warn, Logger};
use std::{collections::BTreeMap, net::SocketAddr, sync::Arc, time::Duration};
use tokio::{
    net::{TcpListener, TcpStream},
    runtime::Runtime,
    sync::{mpsc, RwLock},
};
use tokio_tungstenite::{
    accept_async,
    tungstenite::{Error, Message},
    WebSocketStream,
};

pub type WebsocketSender = mpsc::Sender<WebsocketMessage>;
pub type WebsocketReceiver = mpsc::Receiver<WebsocketMessage>;
pub type WebsocketClient = (WebsocketClientSender, WebsocketClientReceiver);

pub type WebsocketClientSender = mpsc::Sender<Message>;
pub type WebsocketClientReceiver = mpsc::Receiver<Message>;
pub type WebsocketSendError = mpsc::error::TrySendError<WebsocketMessage>;
pub type WebsocketRecvError = mpsc::error::TryRecvError;

pub trait WebsocketService {
    fn message_send(&mut self, message: WebsocketMessage) -> Result<(), WebsocketSendError>;
}

impl WebsocketService for WebsocketServiceDefault {
    fn message_send(&mut self, message: WebsocketMessage) -> Result<(), WebsocketSendError> {
        self.sender.try_send(message)
    }
}

pub struct WebsocketServiceDefault {
    pub sender: WebsocketSender,
    pub connections: Arc<RwLock<BTreeMap<SocketAddr, WebsocketClientSender>>>,
}

impl WebsocketServiceDefault {
    pub fn new(tokio_runtime: &Runtime, bound: usize, websocket_url: String, log: Logger) -> Self {
        let (tx, rx) = mpsc::channel(bound);

        let connections = Arc::new(RwLock::new(BTreeMap::new()));

        let t_log = log.clone();
        let t_connections = connections.clone();
        tokio_runtime.spawn(async move {
            Self::accept_connections(t_connections, websocket_url, t_log).await
        });

        let t_log = log.clone();
        let t_connections = connections.clone();
        tokio_runtime.spawn(async move { Self::run_worker(t_connections, rx, t_log).await });

        WebsocketServiceDefault {
            sender: tx,
            connections,
        }
    }

    async fn accept_connections(
        connections: Arc<RwLock<BTreeMap<SocketAddr, WebsocketClientSender>>>,
        websocket_url: String,
        log: Logger,
    ) {
        let listener = if let Ok(listener) = TcpListener::bind(websocket_url).await {
            info!(log, "Websocket server started");
            listener
        } else {
            error!(log, "Failed to bind ws address");
            return;
        };

        while let Ok((stream, _)) = listener.accept().await {
            if let Ok(peer_addr) = stream.peer_addr() {
                if let Ok(ws_stream) = accept_async(stream).await {
                    let t_log = log.clone();
                    let t_connections = connections.clone();
                    tokio::task::spawn(Self::handle_connection(
                        t_connections,
                        peer_addr,
                        ws_stream,
                        t_log,
                    ));

                    info!(log, "Websocket Client connected"; "Address" => peer_addr);
                } else {
                    // TODO: more descriptive errors
                    warn!(log, "Failed to upgrade to ws protocol");
                };
            } else {
                warn!(log, "Ignoring peer without an address");
                continue;
            };
        }
    }

    async fn handle_connection(
        connections: Arc<RwLock<BTreeMap<SocketAddr, WebsocketClientSender>>>,
        client_address: SocketAddr,
        ws_stream: WebSocketStream<TcpStream>,
        log: Logger,
    ) {
        let (tx, mut rx) = mpsc::channel(100);
        let mut connections_writer = connections.write().await;
        connections_writer.insert(client_address, tx);
        drop(connections_writer);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        let mut interval = tokio::time::interval(Duration::from_millis(1000));

        loop {
            tokio::select! {
                outgoing_msg = rx.recv() => {
                    match outgoing_msg {
                        Some(msg) => {
                            ws_sender.send(msg).await;
                        },
                        None => break,
                    }
                }
                incoming_msg = ws_receiver.next() => {
                    match incoming_msg {
                        Some(msg) => {
                            let msg = msg.unwrap();
                            if msg.is_close() {
                                info!(log, "Websocket Client disconnected"; "Address" => client_address);
                                let mut connections_writer = connections.write().await;
                                connections_writer.remove(&client_address);
                                break;
                            }
                        }
                        None => {
                            break;
                        }
                    }
                }
                _ = interval.tick() => {
                    // TODO (monitoring-refactor): remove
                    ws_sender.send(Message::Text("tick".to_owned())).await;
                }
            }
        }
    }

    async fn run_worker(
        connections: Arc<RwLock<BTreeMap<SocketAddr, WebsocketClientSender>>>,
        mut receiver: WebsocketReceiver,
        log: Logger,
    ) {
        while let Some(msg) = receiver.recv().await {
            crit!(log, "New message propagated");
            let connections = connections.read().await;

            let serialized = match serde_json::to_string(&msg) {
                Ok(json_string) => json_string,
                Err(e) => {
                    warn!(log, "Failed to serialize websocket message: {:?}", e);
                    continue;
                }
            };

            for sender in connections.values() {
                if let Err(e) = sender.send(Message::Text(serialized.clone())).await {
                    warn!(log, "Failed to send message to mpsc channel: {:?}", e);
                }
            }
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum WebsocketMessage {
    PeerStatus(DummyPeerStatusMessage),
    DownloadedBlocksStats(DummyDownloadedBlocksStats),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DummyPeerStatusMessage {
    pub address: SocketAddr,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DummyDownloadedBlocksStats {
    pub count: usize,
}
