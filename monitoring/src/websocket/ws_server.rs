// // Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// // SPDX-License-Identifier: MIT

use std::convert::Infallible;
use std::net::SocketAddr;

use futures::{FutureExt, StreamExt};
use slog::{info, warn, Logger};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::ws::WebSocket;
use warp::Filter;
use warp::{reject, Rejection, Reply};

use crate::websocket::Clients;

pub async fn run_websocket(
    address: SocketAddr,
    max_number_of_websocket_connections: u16,
    clients: Clients,
    log: Logger,
) {
    let ws_route = warp::path::end()
        .and(warp::ws())
        .and(with_clients(clients.clone()))
        .and(with_max_number_of_websocket_connections(
            max_number_of_websocket_connections,
        ))
        .and(with_log(log.clone()))
        .and_then(ws_handler)
        .recover(move |rejection| handle_rejection(rejection, log.clone()))
        .with(warp::cors().allow_any_origin())
        .boxed();

    warp::serve(ws_route).run(address).await
}

fn with_clients(clients: Clients) -> BoxedFilter<(Clients,)> {
    warp::any().map(move || clients.clone()).boxed()
}

fn with_max_number_of_websocket_connections(
    max_number_of_websocket_connections: u16,
) -> BoxedFilter<(u16,)> {
    warp::any()
        .map(move || max_number_of_websocket_connections)
        .boxed()
}

fn with_log(log: Logger) -> BoxedFilter<(Logger,)> {
    warp::any().map(move || log.clone()).boxed()
}

#[derive(Debug)]
struct MaximumNumberOfConnectionExceededError {
    max_number_of_websocket_connections: u16,
    actual_count: usize,
}

impl reject::Reject for MaximumNumberOfConnectionExceededError {}

pub async fn handle_rejection(err: Rejection, log: Logger) -> Result<impl Reply, Infallible> {
    if err.is_not_found() {
        warn!(log, "Websocket handle error"; "message" => "rpc not found");
        Ok(warp::reply::with_status(
            String::from("websocket path not found"),
            StatusCode::NOT_FOUND,
        ))
    } else {
        if let Some(tcre) = err.find::<MaximumNumberOfConnectionExceededError>() {
            warn!(log, "Websocket maximum open connection exceeded";
                       "max_number_of_websocket_connections" => tcre.max_number_of_websocket_connections,
                       "actual_count" => tcre.actual_count);
            Ok(warp::reply::with_status(
                String::from("websocket is temporary unavailable"),
                StatusCode::SERVICE_UNAVAILABLE,
            ))
        } else {
            warn!(log, "Websocket handle error"; "message" => "unhandled error occurred", "detail" => format!("{:?}", err));
            Ok(warp::reply::with_status(
                String::from("unhandled error occurred"),
                StatusCode::INTERNAL_SERVER_ERROR,
            ))
        }
    }
}

pub async fn ws_handler(
    ws: warp::ws::Ws,
    clients: Clients,
    max_number_of_websocket_connections: u16,
    log: Logger,
) -> Result<impl Reply, Rejection> {
    // limit max number of open websockets
    let clients_count = clients.read().await.len();
    if max_number_of_websocket_connections <= clients_count as u16 {
        return Err(MaximumNumberOfConnectionExceededError {
            actual_count: clients_count,
            max_number_of_websocket_connections,
        }
        .into());
    }

    // handle websocket
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
