use std::sync::{Arc, RwLock};
use std::fs;
use std::path::{PathBuf, Path};

use slog::{info, Drain, Level, Logger};

mod configuration;
mod filters;
mod handlers;
mod node_runner;
mod tezos_client_runner;

const TEZOS_CLIENT_DIR: &str = "./tezos-client";

#[tokio::main]
async fn main() {
    // parse and validate program arguments
    let env = configuration::LauncherEnvironment::from_args();

    // create an slog logger
    let log = create_logger(env.log_level);

    // create a thread safe reference to the runner struct
    let runner = Arc::new(RwLock::new(node_runner::LightNodeRunner::new(
        "light-node-0",
        env.light_node_path,
    )));

    // ensure the tezos-client directory does not exist and create a temporary dir for tezos-client data
    if Path::new(TEZOS_CLIENT_DIR).exists() {
        fs::remove_dir_all(TEZOS_CLIENT_DIR).expect("Failed to delete tezos-client directory!");
    }
    fs::create_dir(TEZOS_CLIENT_DIR).expect("Failed to create tezos-client directory");

    // create a thread safe reference to the client runner struct
    let client_runner = Arc::new(RwLock::new(tezos_client_runner::TezosClientRunner::new(
        "tezos-client",
        env.tezos_client_path,
        PathBuf::from(TEZOS_CLIENT_DIR),
    )));

    // the port to open the rpc server on
    let rpc_port = env.sandbox_rpc_port;

    // combined warp filter
    let api = filters::sandbox(log.clone(), runner, client_runner);

    info!(log, "Starting the sandbox RPC server");

    // start serving the api
    warp::serve(api).run(([0, 0, 0, 0], rpc_port)).await;
}

/// Creates a slog Logger
fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .chan_size(32768)
    .overflow_strategy(slog_async::OverflowStrategy::Block)
    .build()
    .filter_level(level)
    .fuse();
    Logger::root(drain, slog::o!())
}
