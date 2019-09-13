#[macro_use]
extern crate lazy_static;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{error, info};
use riker::actors::*;
use tokio::runtime::Runtime;

use networking::p2p::network_channel::NetworkChannel;
use networking::p2p::network_manager::NetworkManager;
use shell::chain_manager::ChainManager;
use shell::peer_manager::{PeerManager, Threshold};
use storage::block_storage::BlockStorage;
use storage::operations_storage::{OperationsMetaStorage, OperationsStorage};
use storage::persistent::{open_db, Schema};

mod configuration;

fn main() {
    let identity_json_file_path: PathBuf = configuration::ENV.identity_json_file_path.clone()
        .unwrap_or_else(|| {
            let tezos_default_identity: PathBuf = configuration::tezos_node::get_default_tezos_identity_json_file_path().unwrap();
            if tezos_default_identity.exists() {
                // if exists tezos default location, then use it
                tezos_default_identity
            } else {
                // or just use our config/identity.json
                Path::new("./config/identity.json").to_path_buf()
            }
        });

    info!("Starting Light Node");

    let identity = configuration::tezos_node::load_identity(identity_json_file_path);
    if let Err(e) = identity {
        error!("Failed to load identity. Reason: {:?}", e);
        return;
    }
    let identity = identity.unwrap();

    let schemas = vec![
        BlockStorage::cf_descriptor(),
        OperationsStorage::cf_descriptor(),
        OperationsMetaStorage::cf_descriptor(),
    ];
    let rocks_db = open_db(&configuration::ENV.bootstrap_db_path, schemas)
        .expect(&format!("Failed to create RocksDB database at '{:?}'", &configuration::ENV.bootstrap_db_path));

    let actor_system = ActorSystem::new().expect("Failed to create actor system");
    let tokio_runtime = Runtime::new().expect("Failed to create tokio runtime");

    let network_channel = NetworkChannel::actor(&actor_system)
        .expect("Failed to create network channel");
    let network_manager = NetworkManager::actor(
        &actor_system,
        network_channel.clone(),
        tokio_runtime.executor(),
        configuration::ENV.p2p.listener_port,
        identity.public_key,
        identity.secret_key,
        identity.proof_of_work_stamp)
        .expect("Failed to create network manager");
    let _ = PeerManager::actor(
        &actor_system,
        network_channel.clone(),
        network_manager.clone(),
        &configuration::ENV.p2p.bootstrap_lookup_addresses,
        &configuration::ENV.initial_peers,
        Threshold::new(2, 30))
        .expect("Failed to create peer manager");
    let _ = ChainManager::actor(
        &actor_system,
        network_channel.clone(),
        Arc::new(rocks_db),
        configuration::ENV.tezos_data_dir.clone()
    );

    tokio_runtime.block_on(async move {
        use tokio::net::signal;
        use futures::future;
        use futures::stream::StreamExt;

        let ctrl_c = signal::ctrl_c().unwrap();
        let prog = ctrl_c.take(1).for_each(|_| {
            info!("ctrl-c received!");
            future::ready(())
        });
        prog.await;
        let _ = actor_system.shutdown().await;
    });
}


