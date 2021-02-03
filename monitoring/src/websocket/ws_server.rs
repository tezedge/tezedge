// // Copyright (c) SimpleStaking and Tezedge Contributors
// // SPDX-License-Identifier: MIT

use crate::websocket::Clients;
use futures::{FutureExt, StreamExt};
use slog::{info, warn, Logger};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::ws::WebSocket;
use warp::{Rejection, Reply};

pub async fn client_connection(ws: WebSocket, clients: Clients, log: Logger) {
    let (client_ws_sender, mut client_ws_rcv) = ws.split();
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    // create an uuid to add to a hashmap
    let id = Uuid::new_v4().to_string();

    clients
        .write()
        .await
        .insert(id.clone(), Some(client_sender));

    // wrap the reciever into a stream
    let client_stream = UnboundedReceiverStream::new(client_rcv);

    let log = log.clone();
    info!(log, "Websocket: {} connected", &id);
    // forward the client stream to the websocket
    tokio::task::spawn({
        client_stream.forward(client_ws_sender).map(move |result| {
            if let Err(e) = result {
                warn!(log, "error sending websocket msg: {}", e);
            }
        })
    });

    while let Some(_) = client_ws_rcv.next().await {
        ( /* we are not expecting anything from the client side, just wait for the connection to close */ )
    }
    clients.write().await.remove(&id);
}

pub async fn ws_handler(
    ws: warp::ws::Ws,
    clients: Clients,
    log: Logger,
) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| client_connection(socket, clients, log)))
}
