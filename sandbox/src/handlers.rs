use std::convert::Infallible;

use slog::{error, info, Logger};
use warp::http::StatusCode;

use crate::node_runner::LightNodeRunnerRef;
use crate::tezos_client_runner::TezosClientRunner;

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
    client_runner: TezosClientRunner,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to stop the light node");

    // aquire a write lock to the runner
    let mut runner = runner.write().unwrap();

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
    log: Logger,
    client_runner: TezosClientRunner,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to init the tezos-client");

    match client_runner.init_client_data() {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error init client, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn activate_protocol(
    log: Logger,
    client_runner: TezosClientRunner,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to activate the protocol");

    match client_runner.activate_protocol() {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error init client, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn bake_block_with_client(
    log: Logger,
    client_runner: TezosClientRunner,
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to bake a block");

    match client_runner.bake_block() {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error init client, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

