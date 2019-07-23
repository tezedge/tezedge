use failure::Error;
use futures::prelude::*;
use http::{Method, Response, StatusCode};
use log::{debug, error, info, warn};
use tokio::net::{TcpStream, TcpListener};

use crate::configuration;
use crate::rpc::RpcTx;
use crate::tezos::p2p::P2pRx;

use super::http::*;
use super::message::{RpcMessage, EmptyMessage};
use super::response::RpcResponse;



async fn process_http_stream(stream: TcpStream, mut rpc_tx: RpcTx) -> Result<(), Error> {
    let (http_rx, http_tx) = stream.split();
    let req = http_receive_request(http_rx).await?;

    // TODO: refactor somehow - mappings needs to come from caller of accept_connections
    let rpc_msg_to_process = match (req.method(), req.uri().path()) {
        (&Method::GET, "/") => {
            http_send_response_ok_text("rp2p rpc is running, see README.md for api desc",http_tx,).await?;
            None
        }
        (&Method::POST, "/network/bootstrap") => {
            let body = &req.body().as_ref().unwrap();
            let msg = serde_json::from_str::<RpcMessage>(&body)?;

            Some(
                (
                    msg,
                    Some(RpcResponse::new(http_tx))
                )
            )
        }
        (&Method::GET, "/network/points") => {
            Some(
                (
                    RpcMessage::NetworkPoints(EmptyMessage {}),
                    Some(RpcResponse::new(http_tx))
                )
            )
        }
        (&Method::GET, "/chains/main/blocks/head") => {
            Some(
                (
                    RpcMessage::ChainsHead(EmptyMessage {}),
                    Some(RpcResponse::new(http_tx))
                )
            )
        }
        _ => {
            warn!("{} {} not found", req.method(), req.uri().path());
            let mut resp = Response::new(None);
            *resp.status_mut() = StatusCode::NOT_FOUND;
            http_send_response(resp, http_tx).await?;
            None
        }
    };

    if let Some((msg, callback)) = rpc_msg_to_process {
        debug!("Pushing RPC message: {:?}", msg);
        rpc_tx.send((msg, callback)).await?;
    }

    Ok(())
}

pub async fn accept_connections(rpc_tx: RpcTx) -> Result<(), Error> {
    let listen_addr = format!("127.0.0.1:{}", configuration::ENV.rpc.listener_port).parse().unwrap();
    let listener = TcpListener::bind(&listen_addr)?;
    let mut incoming = listener.incoming();

    info!("Listening on {:?}", listen_addr);

    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let addr = stream.peer_addr()?;
        let tx = rpc_tx.clone();
        tokio::spawn(async move {
            info!("Accepting stream from: {}", addr);

            match process_http_stream(stream, tx).await {
                Ok(_) => info!("HTTP stream processed"),
                Err(e) => error!("HTTP stream processing failed. Reason: {:?}", e),
            }

            debug!("Closing stream from: {}", addr);
        });
    }

    Ok(())
}

pub async fn forward_p2p_messages_to_rpc(mut p2p_rx: P2pRx) {
    info!("P2P consumer started");

    while let Some(p2p_message) = p2p_rx.next().await {
        debug!("Consuming message P2P message: {:?}", p2p_message);
    }

    info!("P2P consumer stopped");
}
