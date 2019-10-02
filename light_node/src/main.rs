#[macro_use]
extern crate lazy_static;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use log::{debug, error, info};
use riker::actors::*;
use tokio::runtime::Runtime;

use monitoring::{Monitor, WebsocketHandler};
use networking::p2p::network_channel::NetworkChannel;
use networking::p2p::network_manager::NetworkManager;
use shell::chain_feeder::ChainFeeder;
use shell::chain_manager::ChainManager;
use shell::peer_manager::PeerManager;
use shell::shell_channel::ShellChannel;
use storage::{BlockMetaStorage, BlockStorage, initialize_storage_with_genesis_block, OperationsMetaStorage, OperationsStorage};
use storage::persistent::{open_db, Schema};
use tezos_client::client;
use tezos_client::client::{TezosRuntimeConfiguration, TezosStorageInitInfo};

use crate::configuration::tezos_node::Identity;

mod configuration;

macro_rules! shutdown_and_exit {
    ($err:expr, $sys:ident) => {{
        $err;
        futures::executor::block_on($sys.shutdown()).unwrap();
        return;
    }}
}

fn block_on_actors(actor_system: ActorSystem, identity: Identity, init_info: TezosStorageInitInfo, rocks_db: Arc<rocksdb::DB>) {
    let tokio_runtime = Runtime::new().expect("Failed to create tokio runtime");

    let network_channel = NetworkChannel::actor(&actor_system)
        .expect("Failed to create network channel");
    let shell_channel = ShellChannel::actor(&actor_system)
        .expect("Failed to create shell channel");
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
        configuration::ENV.peer_threshold)
        .expect("Failed to create peer manager");
    let _ = ChainManager::actor(&actor_system, network_channel.clone(), shell_channel.clone(), rocks_db.clone(), &init_info)
        .expect("Failed to create chain manager");
    let _ = ChainFeeder::actor(&actor_system, shell_channel.clone(), rocks_db.clone(), &init_info)
        .expect("Failed to create chain feeder");
    let websocket_handler = WebsocketHandler::actor(&actor_system, configuration::ENV.websocket_address)
        .expect("Failed to start websocket actor");
    let _ = Monitor::actor(&actor_system, network_channel.clone(), websocket_handler, shell_channel, rocks_db.clone())
        .expect("Failed to create monitor actor");

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

fn main() {
    let actor_system = ActorSystem::new().expect("Failed to create actor system");
    info!("Starting Light Node");

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

    // setup tezos ocaml runtime
    client::change_runtime_configuration(TezosRuntimeConfiguration { log_enabled: configuration::ENV.ocaml_log_enabled });
    let identity = match configuration::tezos_node::load_identity(identity_json_file_path) {
        Ok(identity) => identity,
        Err(e) => shutdown_and_exit!(error!("Failed to load identity. Reason: {:?}", e), actor_system),
    };
    let tezos_storage_init_info = {
        let tezos_data_dir = &configuration::ENV.tezos_data_dir;
        if !(tezos_data_dir.exists() && tezos_data_dir.is_dir()) {
            shutdown_and_exit!(error!("Required tezos data dir '{:?}' is not a directory or does not exist!", tezos_data_dir), actor_system);
        }
        let tezos_data_dir = tezos_data_dir.to_str().unwrap();
        match client::init_storage(tezos_data_dir.to_string()) {
            Ok(res) => res,
            Err(e) => shutdown_and_exit!(error!("Failed to initialize Tezos OCaml storage in directory '{}'. Reason: {:?}", &tezos_data_dir, e), actor_system)
        }
    };
    debug!("Loaded Tezos constants: {:?}", &tezos_storage_init_info);


    let schemas = vec![
        BlockStorage::cf_descriptor(),
        BlockMetaStorage::cf_descriptor(),
        OperationsStorage::cf_descriptor(),
        OperationsMetaStorage::cf_descriptor(),
    ];
    let rocks_db = match open_db(&configuration::ENV.bootstrap_db_path, schemas) {
        Ok(db) => Arc::new(db),
        Err(_) => shutdown_and_exit!(error!("Failed to create RocksDB database at '{:?}'", &configuration::ENV.bootstrap_db_path), actor_system)
    };
    debug!("Loaded RocksDB database");

    match initialize_storage_with_genesis_block(&tezos_storage_init_info.genesis_block_header_hash, &tezos_storage_init_info.genesis_block_header, rocks_db.clone()) {
        Ok(_) => block_on_actors(actor_system, identity, tezos_storage_init_info, rocks_db.clone()),
        Err(e) => shutdown_and_exit!(error!("Failed to initialize storage with genesis block. Reason: {}", e), actor_system),
    }

    rocks_db.flush().expect("Failed to flush database");
}


