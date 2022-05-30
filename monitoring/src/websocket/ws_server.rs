// // Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// // SPDX-License-Identifier: MIT

use std::convert::Infallible;
use std::net::SocketAddr;

use futures::SinkExt;
use futures::{FutureExt, StreamExt};
use rpc::RpcServiceEnvironmentRef;
use slog::{info, warn, Logger};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;
use warp::filters::BoxedFilter;
use warp::http::StatusCode;
use warp::ws::{Message, WebSocket};
use warp::Filter;
use warp::{reject, Rejection, Reply};

use crate::websocket::ws_json_rpc::{handle_request, JsonRpcError, JsonRpcResponse, Params};
use crate::websocket::Clients;

use super::RpcClients;

pub async fn run_websocket(
    address: SocketAddr,
    max_number_of_websocket_connections: u16,
    monitoring_clients: Clients,
    rpc_clients: RpcClients,
    rpc_env: RpcServiceEnvironmentRef,
    log: Logger,
) {
    let ws_log = log.clone();
    let ws_route = warp::path::end()
        .and(warp::ws())
        .and(with_clients(monitoring_clients.clone()))
        .and(with_max_number_of_websocket_connections(
            max_number_of_websocket_connections,
        ))
        .and(with_log(ws_log.clone()))
        .and_then(ws_handler)
        .recover(move |rejection| handle_rejection(rejection, ws_log.clone()))
        .with(warp::cors().allow_any_origin())
        .boxed();

    let ws_log = log.clone();
    let json_rpc_route = warp::path::path("rpc")
        .and(warp::path::end())
        .and(warp::ws())
        .and(with_rpc_clients(rpc_clients.clone()))
        .and(with_max_number_of_websocket_connections(
            max_number_of_websocket_connections,
        ))
        .and(with_log(log.clone()))
        .and(with_rpc_env(rpc_env))
        .and_then(ws_rpc_handler)
        .recover(move |rejection| handle_rejection(rejection, ws_log.clone()))
        .with(warp::cors().allow_any_origin())
        .boxed();

    let router = json_rpc_route.or(ws_route);

    warp::serve(router).run(address).await
}

fn with_clients(clients: Clients) -> BoxedFilter<(Clients,)> {
    warp::any().map(move || clients.clone()).boxed()
}

fn with_rpc_clients(clients: RpcClients) -> BoxedFilter<(RpcClients,)> {
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

fn with_rpc_env(env: RpcServiceEnvironmentRef) -> BoxedFilter<(RpcServiceEnvironmentRef,)> {
    warp::any().map(move || env.clone()).boxed()
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
    } else if let Some(tcre) = err.find::<MaximumNumberOfConnectionExceededError>() {
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

pub async fn ws_rpc_handler(
    ws: warp::ws::Ws,
    clients: RpcClients,
    max_number_of_websocket_connections: u16,
    log: Logger,
    env: RpcServiceEnvironmentRef,
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
    Ok(ws.on_upgrade(move |socket| client_connection_rpc(socket, clients, log, env)))
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

pub async fn client_connection_rpc(
    ws: WebSocket,
    clients: RpcClients,
    log: Logger,
    env: RpcServiceEnvironmentRef,
) {
    let (client_sender, client_rcv) = mpsc::unbounded_channel();

    // create an uuid to add to a hashmap
    let id = Uuid::new_v4().to_string();
    clients
        .write()
        .await
        .insert(id.clone(), Some(client_sender.clone()));
    info!(log, "New websocket connection detected"; "id" => &id);

    // redirect channel to websocket
    {
        let log = log.clone();
        let id = id.clone();

        let (mut user_ws_tx, mut user_ws_rx) = ws.split();

        let mut rx = UnboundedReceiverStream::new(client_rcv);

        let t_log = log.clone();
        tokio::task::spawn(async move {
            while let Some(message) = rx.next().await {
                let message = serde_json::to_string(&message).unwrap();
                if let Err(e) = user_ws_tx.send(Message::text(message)).await {
                    slog::error!(t_log, "Websocket error: {}", e);
                }
            }
        });

        while let Some(result) = user_ws_rx.next().await {
            let msg = match result {
                Ok(message) => message,
                Err(e) => {
                    slog::error!(
                        log,
                        "Failed to recieve message from websocket channel: user {id}, error: {e}"
                    );
                    break;
                }
            };

            if let Ok(msg) = msg.to_str() {
                let response =
                    match serde_json::from_str::<json_rpc_types::Request<Params, String>>(msg) {
                        Ok(request) => handle_request(&request, &env).await,
                        Err(_) => {
                            let error =
                                JsonRpcError::from_code(json_rpc_types::ErrorCode::ParseError);
                            JsonRpcResponse::error(json_rpc_types::Version::V2, error, None)
                        }
                    };
                if let Err(e) = client_sender.send(response) {
                    slog::error!(
                        log,
                        "Failed to send to websocket channel: user {id}, error: {e}"
                    );
                }
            }
        }
    }

    info!(log, "Websocket connection removed"; "id" => &id);
    clients.write().await.remove(&id);
}
