use failure::Error;
use futures::prelude::*;
use hyper::{self, Body, Method, Request, Response, Server, StatusCode, header};

use hyper::server::conn::AddrStream;
use hyper::service::{make_service_fn, service_fn};
use serde_json;
use serde_json::json;

use log::{info, warn};

use crate::rpc::message::RpcMessage;
use crate::p2p::node::P2pLayer;
use crate::p2p::message::JsonMessage;


async fn process_http_request(request: Request<Body>, p2p: P2pLayer) -> Result<Response<Body>, Error> {
    let response = match (request.method(), request.uri().path()) {
        (&Method::GET, "/") => {
            Response::builder()
                .header(header::CONTENT_TYPE, "text/html")
                .status(StatusCode::OK)
                .body(Body::from("rp2p rpc is running, see README.md for api desc"))
                .unwrap()
        }
        (&Method::POST, "/network/bootstrap") => {
            let body = request.into_body().try_concat().await?;
            let body = String::from_utf8(body.to_vec())?;
            let msg = serde_json::from_str::<RpcMessage>(&body)?;

            match msg {
                RpcMessage::BootstrapWithPeers(peers) => {
                    p2p.bootstrap_with_peers(peers).await?;
                    Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .status(StatusCode::OK)
                        .body(Body::from(
                            json!({
                                "result": "async bootstrap starts - see log or /network/points"
                            }).to_string()))
                        .unwrap()
                }
                RpcMessage::BootstrapWithLookup(_) => {
                    p2p.bootstrap_with_lookup().await?;
                    Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .status(StatusCode::OK)
                        .body(Body::from(
                            json!({
                                "result": "async bootstrap starts (with lookup) - see log or /network/points"
                            }).to_string()))
                        .unwrap()
                }
                _ => Response::builder()
                        .header(header::CONTENT_TYPE, "application/json")
                        .status(StatusCode::UNPROCESSABLE_ENTITY)
                        .body(Body::from(
                            json!({
                                "result": "operation is not supported"
                            }).to_string()))
                        .unwrap()
            }
        }
        (&Method::GET, "/network/points") => {
            let network_points = p2p.get_network_points().await;
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .status(StatusCode::OK)
                .body(Body::from(
                    json!({
                        "result": network_points
                    }).to_string()))
                .unwrap()
        }
        (&Method::GET, "/chains/main/blocks/head") => {
            match p2p.get_chains_head().await {
                Some(head) => Response::builder()
                    .header(header::CONTENT_TYPE, "application/json")
                    .status(StatusCode::OK)
                    .body(Body::from(
                        json!({
                            "result": {
                                "chain_id": head.chain_id(),
                                "hash": "TODO",
                                "header": head.header().as_json()?,
                                "metadata": {},
                                "operations": []
                            }
                        }).to_string()))
                    .unwrap(),
                None =>  Response::builder()
                    .header(header::CONTENT_TYPE, "application/json")
                    .status(StatusCode::OK)
                    .body(Body::from(
                        json!({
                            "result": []
                        }).to_string()))
                    .unwrap()
            }
        }
        _ => {
            warn!("RPC endpoint {} {} not found", request.method(), request.uri().path());
            Response::builder()
                .header(header::CONTENT_TYPE, "application/json")
                .status(StatusCode::NOT_FOUND)
                .body(Body::empty())
                .unwrap()
        }
    };

    Ok(response)
}

pub async fn accept_connections(p2p: P2pLayer, listener_port: u16) -> Result<(), Error> {
    let listen_address = format!("127.0.0.1:{}", listener_port).parse()?;

    let service = make_service_fn(move |_: &AddrStream| {
        let p2p = p2p.clone();
        async move {
            let p2p = p2p.clone();
            Ok::<_, Error>(service_fn(move |req: Request<Body>| {
                process_http_request(req, p2p.clone())
            }))
        }
    });

    let server = Server::bind(&listen_address)
        .serve(service);

    info!("Listening on http://{}", listen_address);

    server.await?;

    Ok(())
}

