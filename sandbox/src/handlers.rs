use std::convert::Infallible;

use slog::{error, info, Logger};
use warp::http::StatusCode;

use crate::node_runner::LightNodeRunnerRef;
use crate::tezos_client_runner::{TezosClientRunnerRef, TezosProtcolActivationParameters, SandboxWallets, BakeRequest};

/// Handler for start endpoint
pub async fn start_node_with_config(
    cfg: serde_json::Value,
    log: Logger,
    runner: LightNodeRunnerRef,
) -> Result<impl warp::Reply, Infallible> {
    info!(
        log,
        "Received request to start the light node with config: {:?}", cfg
    );

    // aquire a write lock to the runner
    let mut runner = runner.write().unwrap();

    // spawn the node
    match runner.spawn(cfg) {
        Ok(()) => {
            info!(log, "Light node started successfully");
            Ok(StatusCode::OK)
        }
        Err(e) => {
            error!(log, "Cannot start light-node process, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn stop_node(
    log: Logger,
    runner: LightNodeRunnerRef,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to stop the light node");

    // aquire a write lock to the runner
    let mut runner = runner.write().unwrap();

    let client_runner = client_runner.read().unwrap();

    // cleanup tezos client data
    let _ = client_runner.cleanup();

    // shut down the node
    match runner.shut_down() {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error stopping the node, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn init_client_data(
    wallets: SandboxWallets,
    log: Logger,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to init the tezos-client");

    let mut client_runner = client_runner.write().unwrap();

    match client_runner.init_client_data(wallets) {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error init client, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn activate_protocol(
    activation_parameters: TezosProtcolActivationParameters,
    log: Logger,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to activate the protocol");

    println!("PARAMS: {:?}", activation_parameters);

    let client_runner = client_runner.read().unwrap();

    match client_runner.activate_protocol(activation_parameters) {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error init client, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn bake_block_with_client(
    request: BakeRequest,
    log: Logger,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to bake a block");

    let client_runner = client_runner.read().unwrap();

    match client_runner.bake_block(request) {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error init client, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

