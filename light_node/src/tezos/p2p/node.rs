use std::sync::Arc;

use failure::Error;
use futures::lock::Mutex;
use futures::prelude::*;
use futures::stream::futures_unordered::FuturesUnordered;
use log::{debug, error, info, warn};
use serde_json::json;
use serde_json::Value;
use tokio;

use tezos_encoding::hash::{prefix, to_prefixed_hash};

use crate::configuration::tezos_node;
use crate::rpc::http::*;
use crate::rpc::message::*;
use crate::rpc::RpcRx;
use crate::tezos::p2p::message::JsonMessage;
use crate::tezos::p2p::pool::P2pPool;

use super::client::P2pClient;
use super::peer::P2pPeer;
use crate::configuration;

/// node - represents running rust tezos node, node communicates with remote peers

pub async fn forward_rpc_messages_to_p2p(
    mut rpc_rx: RpcRx,
    p2p_client: P2pClient) {
    info!("RPC consumer started");

    let pool = Arc::new(Mutex::new(P2pPool::new()));
    let p2p_client = Arc::new(p2p_client);

    while let Some(rpc_message) = rpc_rx.next().await {
        let p2p_client = p2p_client.clone();
        let pool = pool.clone();

        tokio::spawn(async move {
            let res = match rpc_message {
                (RpcMessage::BootstrapWithPeers(msg), rpc_callback) => {
                    debug!("Handling rpc call BootstrapWithPeers...");

                    // rpc response, if needed at first
                    if let Some(rpc_callback) = rpc_callback {
                        match http_send_response_ok_json(r#"{ "result": "async bootstrap starts - see log or /network/points" }"#, rpc_callback).await {
                            Ok(_) => debug!("HTTP response sent"),
                            Err(e) => error!("Send HTTP response failed. Reason: {:?}", e),
                        }
                    }

                    let initial_peers: Vec<PeerURL> = msg.initial_peers;
                    debug!("BootstrapWithPeers - initial_peers:{:?}", initial_peers);

                    // than start async bootstraping with received peers
                    bootstrap(&p2p_client, &initial_peers, pool).await
                }
                (RpcMessage::BootstrapWithLookup(_), rpc_callback) => {
                    debug!("Handling rpc call BootstrapWithLookup...");

                    // rpc response, if needed at first
                    if let Some(rpc_callback) = rpc_callback {
                        match http_send_response_ok_json(r#"{ "result": "async bootstrap starts - see log or /network/points" }"#, rpc_callback).await {
                            Ok(_) => debug!("HTTP response sent"),
                            Err(e) => error!("Send HTTP response failed. Reason: {:?}", e)
                        }
                    }

                    let initial_peers = tezos_node::lookup_initial_peers(&configuration::ENV.p2p.bootstrap_lookup_address).unwrap();
                    debug!("BootstrapWithLookup({:?}) - initial_peers:{:?}", &configuration::ENV.p2p.bootstrap_lookup_address, initial_peers);

                    // than start async bootstrapping with peers from lookup
                    bootstrap(&p2p_client, &initial_peers, pool.clone()).await
                }
                (RpcMessage::NetworkPoints(_), rpc_callback) => {
                    debug!("Handling rpc call NetworkPoints...");

                    let peers_as_json = pool.lock().await.get_network_peer_as_json();
                    debug!("result: {:?}", peers_as_json);

                    // rpc response
                    let rpc_callback = rpc_callback.expect("Callback expected!");
                    let response_json = format!(r#"[{}]"#, peers_as_json);
                    let send_res = http_send_response_ok_json(&response_json, rpc_callback).await;

                    debug!("...handling rpc call NetworkPoints done!");
                    send_res
                }
                (RpcMessage::ChainsHead(_), rpc_callback) => {
                    debug!("Handling rpc call ChainsHead...");

                    let (header_as_json, chain_id): (Value, String) = match p2p_client.get_current_branch() {
                        None => (
                            serde_json::from_str(&String::from("{}")).unwrap(),
                            String::from("")
                        ),
                        Some(branch) => (
                            serde_json::from_str(&branch.get_current_branch().get_current_head().as_json().unwrap()).unwrap(),
                            to_prefixed_hash(&prefix::CHAIN_ID, branch.get_chain_id().clone())
                        )
                    };

                    let json = json!({
                    // TODO: TEZ-22 (Demo)
//                        "protocol" : "TODO: ",
                        "chain_id" : chain_id,
                        // TODO: TEZ-22 (Demo)
//                        "hash" : "TODO: ",
                        "header" : header_as_json,
                        "metadata" : {}, // TODO: TEZ-22 (Demo)
                        "operations" : [] // TODO: TEZ-22 (Demo)
                    });
                    debug!("json: \n {:?}", json);

                    // rpc response
                    let rpc_callback = rpc_callback.expect("Callback expected!");
                    let response_json = json.to_string();
                    let send_res = http_send_response_ok_json(&response_json, rpc_callback).await;

                    debug!("...handling rpc call ChainsHead done!");
                    send_res
                }
            };

            match res {
                Ok(_) => debug!("Message consumed successfully"),
                Err(e) => error!("Failed to consume message. Reason: {:?}", e),
            }
        });
    }

    info!("RPC consumer stopped");
}

async fn bootstrap<'a>(
    p2p_client: &'a P2pClient,
    peers: &'a Vec<PeerURL>,
    pool: Arc<Mutex<P2pPool>>) -> Result<(), Error> {

    let mut bootstrap_futures = FuturesUnordered::new();
    for peer in peers {
        bootstrap_futures.push(p2p_client.connect_peer(&peer));
    }

    while let Some(peer_bootstrap) = bootstrap_futures.next().await {
        match peer_bootstrap {
            Ok(peer_bootstrap) => {
                info!("Bootstrap of {:?} successful", hex::encode(peer_bootstrap.get_public_key()));

                let peer_bootstrap = Arc::new(peer_bootstrap);
                pool.lock().await.insert_peer(peer_bootstrap.get_peer_id(), peer_bootstrap.clone());

                // start peer processing
                tokio::spawn(
                    accept_peer_data(
                        p2p_client.clone(),
                        peer_bootstrap.clone(),
                    )
                );
            },
            Err(ref e) => error!("Bootstrap failed. Reason: {:?}", e),
        }
    }

    Ok(())
}

async fn accept_peer_data(p2p_client: P2pClient, peer: Arc<P2pPeer>) {
    info!("Initialize p2p business with peer: {}", peer.get_peer_id());
    p2p_client.start_p2p_biznis(&peer).await;

    info!("Starting accepting messages from peer: {}", peer.get_peer_id());

    while let Ok(msg) = peer.read_message().await {
        match p2p_client.handle_message(&peer, &msg).await {
            Ok(()) => info!("Message processed successfully"),
            Err(e) => warn!("Failed to process received message: {:?}", e)
        }
    }
}
