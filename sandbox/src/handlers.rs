use std::convert::Infallible;
use std::error::Error;

use serde::Serialize;
use slog::{info, Logger};
use warp::http::StatusCode;
use warp::{reject, Rejection, Reply};

use itertools::Itertools;

use crate::node_runner::{LightNodeRunnerError, LightNodeRunnerRef};
use crate::tezos_client_runner::{
    BakeRequest, SandboxWallets, TezosClientRunnerError, TezosClientRunnerRef,
    TezosProtcolActivationParameters, TezosClientReply, TezosClientErrorReply,
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

    #[serde(skip_serializing_if = "Option::is_none")]
    field_name: Option<String>,
}

pub async fn handle_rejection(err: Rejection) -> Result<impl Reply, Infallible> {
    let code;
    let message;
    let mut field_name: Option<String> = None;

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
        field_name = extract_field_name(reason);
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
        field_name,
    });

    Ok(warp::reply::with_status(json, code))
}

fn reply_with_clinet_output(output: String) -> Result<impl warp::Reply, reject::Rejection> {
    
    if let Some((field_name, message)) = extract_field_name_and_message_ocaml(&output) {
        let json = warp::reply::json(&TezosClientErrorReply{
            message: message.to_string(),
            field_name,
        });
        Ok(warp::reply::with_status(json, StatusCode::INTERNAL_SERVER_ERROR))
    } else {
        let json = warp::reply::json(&TezosClientReply {
            message: output,
        });
        Ok(warp::reply::with_status(json, StatusCode::OK))
    }
}

fn extract_field_name(message: &str) -> Option<String> {

    let field_name = message.split_whitespace().filter(|s| s.starts_with("\'--")).map(|s| s.to_string()).collect::<Vec<String>>();

    if field_name.len() < 1 {
        None
    } else {
        Some(field_name[0].replace("\'--", ""))
    }
}

fn extract_field_name_and_message_ocaml(message: &str) -> Option<(String, String)>{

    let parsed_message = message.split(",").filter(|s| s.contains("Invalid protocol_parameters: At /")).map(|s| s.to_string()).join("");
    let field_name: Vec<&str> = parsed_message.split_whitespace().collect();

    println!("FN: {:?}", field_name);

    let field_name = parsed_message.split_whitespace().last();

    println!("FN: {:?}", field_name);
    println!("MESSAGE: {:?}", message);

    if let Some(field_name) = field_name {
        let parsed_message = message.split(",").collect::<Vec<&str>>().join("");
        let message = parsed_message.split("\\n").collect::<Vec<&str>>();


        Some((field_name.to_string().replace("/", ""), message[0].to_string()))
    } else {
        None
    }
}
