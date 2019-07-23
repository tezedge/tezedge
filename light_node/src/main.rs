#![feature(async_await)]
#[macro_use]
extern crate lazy_static;

use std::path::{Path, PathBuf};

use futures::channel::mpsc;
use futures::prelude::*;
use log::{debug, error, info};
use tokio;

use crate::rpc::message::BootstrapMessage;
use crate::rpc::message::EmptyMessage;
use crate::rpc::message::PeerURL;
use crate::tezos::storage::db::Db;

mod tezos;
mod rpc;
mod configuration;

const LOG_FILE: &str = "log4rs.yml";
pub const MPSC_BUFFER_SIZE: usize = 50;

/// Function configures default console logger.
fn configure_default_logger() {
    use log::LevelFilter;
    use log4rs::append::console::ConsoleAppender;
    use log4rs::encode::pattern::PatternEncoder;
    use log4rs::config::{Appender, Config, Root};

    let stdout = ConsoleAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} {h({l})} {t} - {h({m})} {n}")))
        .build();

    let config = Config::builder()
        .appender(Appender::builder().build("stdout", Box::new(stdout)))
        .build(Root::builder().appender("stdout").build(LevelFilter::Info))
        .unwrap();

    log4rs::init_config(config).unwrap();
}

#[tokio::main]
async fn main() {
    use crate::tezos::p2p::client::P2pClient;

    match log4rs::init_file(LOG_FILE, Default::default()) {
        Ok(_) => debug!("Logger configured from file: {}", LOG_FILE),
        Err(m) => {
            println!("Logger configuration file {} not loaded: {}", LOG_FILE, m);
            println!("Using default logger configuration");
            configure_default_logger()
        }
    }

    let initial_peers: Vec<PeerURL> = configuration::ENV.initial_peers.clone()
        .into_iter()
        .map(|(ip, port)| {
            PeerURL {
                host: ip.clone(),
                port: port.clone(),
            }
        })
        .collect();

    let identity_json_file_path: PathBuf = configuration::ENV.identity_json_file_path.clone()
        .unwrap_or_else(|| {
            let tezos_default_identity: PathBuf = configuration::tezos_node::get_default_tezos_identity_json_file_path().unwrap();
            if tezos_default_identity.exists() {
                // if exists tezos default location, then use it
                tezos_default_identity
            } else {
                // or just use our config/identity.json
                let tezos_default_identity = Path::new("./config/identity.json").to_path_buf();
                tezos_default_identity
            }
        });

    info!("Starting Iron p2p");

    let (mut rpc_tx, rpc_rx) = mpsc::channel(MPSC_BUFFER_SIZE);
    let (_, p2p_rx) = mpsc::channel(MPSC_BUFFER_SIZE);

    let init_chain_id = hex::decode(configuration::tezos_node::genesis_chain_id());
    if let Err(e) = init_chain_id {
        error!("Failed to load initial chain id. Reason: {:?}", e);
        return;
    }
    let identity = configuration::tezos_node::load_identity(identity_json_file_path);
    if let Err(e) = identity {
        error!("Failed to load identity. Reason: {:?}", e);
        return;
    }

    let p2p_client = P2pClient::new(
        init_chain_id.unwrap(),
        identity.unwrap(),
        configuration::tezos_node::versions(),
        Db::new()
    );

    tokio::spawn(
        tezos::p2p::node::forward_rpc_messages_to_p2p(
            rpc_rx,
            p2p_client.clone()
        )
    );
    tokio::spawn(rpc::server::forward_p2p_messages_to_rpc(p2p_rx));

    // init node bootstrap
    if initial_peers.is_empty() == false {
        rpc_tx.send((rpc::message::RpcMessage::BootstrapWithPeers(BootstrapMessage { initial_peers }), None)).await
            .expect("Failed to transmit bootstrap message to p2p layer")
    } else {
        rpc_tx.send((rpc::message::RpcMessage::BootstrapWithLookup(EmptyMessage {}), None)).await
            .expect("Failed to transmit bootstrap message to p2p layer")
    }

    // ------------------
    // Lines after the following block will be executed only after accept_connections() task will complete
    // ------------------
    let res = rpc::server::accept_connections(rpc_tx.clone()).await;
    if let Err(e) = res {
        error!("Failed to start accepting RPC connections. Reason: {:?}", e);
        return;
    }



    info!("Iron p2p stopped")
}
