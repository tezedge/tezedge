use std::convert::Infallible;
use std::error::Error;

use serde::Serialize;
use slog::{info, Logger};
use warp::http::StatusCode;
use warp::{reject, Rejection, Reply};

use crate::node_runner::{LightNodeRunnerError, LightNodeRunnerRef};
use crate::tezos_client_runner::{
    BakeRequest, SandboxWallets, TezosClientRunnerError, TezosClientRunnerRef,
    TezosProtcolActivationParameters, TezosClientRepply,
};

/// Handler for start endpoint
pub async fn start_node_with_config(
    cfg: serde_json::Value,
    log: Logger,
    runner: LightNodeRunnerRef,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(
        log,
        "Received request to start the light node with config: {:?}", cfg
    );

    // aquire a write lock to the runner
    let mut runner = runner.write().unwrap();

    info!(log, "Starting light-node...");

    // spawn the node
    runner.spawn(cfg)?;

    Ok(StatusCode::OK)
}

pub async fn stop_node(
    log: Logger,
    runner: LightNodeRunnerRef,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to stop the light node");

    // aquire a write lock to the runner
    let mut runner = runner.write().unwrap();

    let client_runner = client_runner.read().unwrap();

    // cleanup tezos client data
    let _ = client_runner.cleanup();

    // shut down the node
    runner.shut_down()?;

    Ok(StatusCode::OK)
}

pub async fn init_client_data(
    wallets: SandboxWallets,
    log: Logger,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to init the tezos-client");

    let mut client_runner = client_runner.write().unwrap();

    let client_output = client_runner.init_client_data(wallets)?;

    reply_with_clinet_output(client_output)
}

pub async fn activate_protocol(
    activation_parameters: TezosProtcolActivationParameters,
    log: Logger,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to activate the protocol");

    let client_runner = client_runner.read().unwrap();

    let client_output = client_runner.activate_protocol(activation_parameters)?;

    reply_with_clinet_output(client_output)
}

pub async fn bake_block_with_client(
    request: BakeRequest,
    log: Logger,
    client_runner: TezosClientRunnerRef,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to bake a block");

    let client_runner = client_runner.read().unwrap();

    let client_output = client_runner.bake_block(request)?;

    reply_with_clinet_output(client_output)
}

#[derive(Serialize)]
struct ErrorMessage {
    code: u16,
    message: String,
}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "NOT FOUND";
    } else if let Some(TezosClientRunnerError::ProtocolParameterError) = err.find() {
        code = StatusCode::BAD_REQUEST;
        message = "Protocol parameter deserialization error";
    } else if let Some(TezosClientRunnerError::NonexistantWallet) = err.find() {
        code = StatusCode::BAD_REQUEST;
        message = "The provided alias is not a known wallet";
    } else if let Some(LightNodeRunnerError::NodeAlreadyRunning) = err.find() {
        code = StatusCode::BAD_REQUEST;
        message = "Node is allready running";
    } else if let Some(LightNodeRunnerError::NodeNotRunnig) = err.find() {
        code = StatusCode::BAD_REQUEST;
        message = "Node not running";
    } else if let Some(LightNodeRunnerError::NodeStartupError {reason}) = err.find() {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = reason;
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        // This error happens if the body could not be deserialized correctly
        match e.source() {
            Some(_) => {
                message = "Request deserialization errror";
            }
            None => message = "Request deserialization errror",
        }
        code = StatusCode::BAD_REQUEST;
    } else {
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "UNHANDLED_REJECTION";
    }

    let json = warp::reply::json(&ErrorMessage {
        code: code.as_u16(),
        message: message.into(),
    });

    Ok(warp::reply::with_status(json, code))
}

fn reply_with_clinet_output(output: String) -> Result<impl warp::Reply, reject::Rejection> {
    let json = warp::reply::json(&TezosClientRepply {
        message: output,
    });

    Ok(warp::reply::with_status(json, StatusCode::OK))
}
