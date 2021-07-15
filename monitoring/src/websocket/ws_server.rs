// // Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// // SPDX-License-Identifier: MIT

use std::convert::Infallible;
use std::net::SocketAddr;

use futures::{FutureExt, StreamExt};
use slog::{info, warn, Logger};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::ws::WebSocket;
use warp::Filter;
use warp::{Rejection, Reply};

use crate::websocket::Clients;

pub async fn run_websocket(address: SocketAddr, clients: Clients, log: Logger) {
    let ws_route = warp::path::end()
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and(with_log(log.clone()))
        .and_then(ws_handler)
        .with(warp::cors().allow_any_origin());

    warp::serve(ws_route).run(address).await
}

fn with_clients(clients: Clients) -> impl Filter<Extract = (Clients,), Error = Infallible> + Clone {
    warp::any().map(move || clients.clone())
}

fn with_log(log: Logger) -> impl Filter<Extract = (Logger,), Error = Infallible> + Clone {
    warp::any().map(move || log.clone())
}

pub async fn ws_handler(
    ws: warp::ws::Ws,
    clients: Clients,
    log: Logger,
) -> Result<impl Reply, Rejection> {
    Ok(ws.on_upgrade(move |socket| client_connection(socket, clients, log)))
}

pub async fn client_connection(ws: WebSocket, clients: Clients, log: Logger) {
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    // create an uuid to add to a hashmap
    let id = Uuid::new_v4().to_string();
    clients
        .write()
        .await
        .insert(id.clone(), Some(client_sender));
    info!(log, "New websocket connection detected"; "id" => &id);

    // redirect channel to websocket
    {
        let log = log.clone();
        let id = id.clone();

        // wrap the reciever into a stream
        UnboundedReceiverStream::new(client_rcv).forward(ws).map(move |result| {
            if let Err(e) = result {
                warn!(log, "Error sending message to connected websocket"; "id" => &id, "reason" => format!("{}", e));
            }
        }).await;
    }

    info!(log, "Websocket connection removed"; "id" => &id);
    clients.write().await.remove(&id);
}
