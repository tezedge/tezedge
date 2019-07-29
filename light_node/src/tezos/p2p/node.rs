use std::sync::Arc;

use failure::Error;
use futures::lock::Mutex;
use futures::prelude::*;
use futures::stream::futures_unordered::FuturesUnordered;
use log::{debug, error, info, warn};
use tokio;

use tezos_encoding::hash::{prefix, to_prefixed_hash};

use crate::configuration::tezos_node;
use crate::rpc::message::*;
use crate::tezos::p2p::message::BlockHeader;
use crate::tezos::p2p::pool::P2pPool;

use super::client::P2pClient;
use super::peer::P2pPeer;
use crate::configuration;

/// node - represents running rust tezos node, node communicates with remote peers

#[derive(Clone)]
pub struct P2pLayer {
    pool: Arc<Mutex<P2pPool>>,
    client: P2pClient,
}

pub struct ChainsHead {
    chain_id: String,
    header: BlockHeader,
}

impl ChainsHead {
    pub fn chain_id(&self) -> &str {
        &self.chain_id
    }

pub fn header(&self) -> &BlockHeader {
        &self.header
    }
}


impl P2pLayer {
    pub fn new(client: P2pClient) -> Self {
        P2pLayer {
            pool: Arc::new(Mutex::new(P2pPool::new())),
            client,
        }
    }

    pub async fn bootstrap_with_peers(&self, msg: BootstrapMessage) -> Result<(), Error> {
        debug!("BootstrapWithPeers - initial_peers:{:?}", &msg.initial_peers);
        bootstrap(&self.client, &msg.initial_peers, self.pool.clone()).await
    }

    pub async fn bootstrap_with_lookup(&self) -> Result<(), Error> {
        let initial_peers = tezos_node::lookup_initial_peers(&configuration::ENV.p2p.bootstrap_lookup_address).unwrap();
        debug!("BootstrapWithLookup({:?}) - initial_peers:{:?}", &configuration::ENV.p2p.bootstrap_lookup_address, &initial_peers);
        bootstrap(&self.client, &initial_peers, self.pool.clone()).await
    }

    pub async fn get_network_points(&self) -> Vec<PeerAddress> {
        self.pool.lock().await.get_peers_addresses()
    }

    pub async fn get_chains_head(&self) -> Option<ChainsHead> {
        match self.client.get_current_branch() {
            Some(current_branch_msg) => Some(ChainsHead {
                chain_id: to_prefixed_hash(&prefix::CHAIN_ID, current_branch_msg.get_chain_id().clone()),
                header: current_branch_msg.get_current_branch().get_current_head().clone(),
            }),
            None => None
        }
    }
}

async fn bootstrap<'a>(
    p2p_client: &'a P2pClient,
    peers: &'a Vec<PeerAddress>,
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
            }
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
