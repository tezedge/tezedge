use std::convert::Infallible;

use slog::{error, info, Logger};
use warp::http::StatusCode;

use crate::node_runner::{LightNodeRunnerRef};

// TODO: discussion about what status codes to return on errors

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
) -> Result<impl warp::Reply, Infallible> {
    info!(log, "Received request to stop the light node");

    // aquire a write lock to the runner
    let mut runner = runner.write().unwrap();

    // shut down the node
    match runner.shut_down() {
        Ok(()) => Ok(StatusCode::OK),
        Err(e) => {
            error!(log, "Error stopping the node, reason: {}", e);
            Ok(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
