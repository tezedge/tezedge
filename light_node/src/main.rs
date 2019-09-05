#[macro_use]
extern crate lazy_static;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use log::{debug, error, info};
use riker::actors::*;
use tokio;

use networking::p2p::network_channel::NetworkChannel;
use networking::p2p::network_manager::NetworkManager;
use shell::chain_manager::ChainManager;
use shell::peer_manager::{PeerManager, Threshold};

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

fn main() {

//    match log4rs::init_file(LOG_FILE, Default::default()) {
//        Ok(_) => debug!("Logger configured from file: {}", LOG_FILE),
//        Err(m) => {
//            println!("Logger configuration file {} not loaded: {}", LOG_FILE, m);
//            println!("Using default logger configuration");
//            configure_default_logger()
//        }
//    }

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

    // -----------------------------
    let actor_system = ActorSystem::new().expect("Failed to create actor system");
    let network_channel = NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
    let network_manager = NetworkManager::actor(
        &actor_system,
        network_channel.clone(),
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
        Threshold::new(1, 30))
        .expect("Failed to create peer manager");
    let _ = ChainManager::actor(&actor_system, network_channel.clone());
    // -----------------------------

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting Ctrl-C handler");
    info!("Waiting for Ctrl-C...");
    while running.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(500));
    }
    info!("Got it! Exiting...");
}


