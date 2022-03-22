// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::Infallible;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::{collections::HashSet, sync::PoisonError};

use itertools::Itertools;
use serde::Serialize;
use slog::{error, info, Logger};
use warp::http::StatusCode;
use warp::{reject, Rejection, Reply};

use crate::node_runner::{LightNodeRunnerError, LightNodeRunnerRef, NodeRpcIpPort};
use crate::tezos_client_runner::{
    reply_with_client_output, BakeRequest, SandboxWallets, TezosClientRunnerError,
    TezosClientRunnerRef, TezosProtcolActivationParameters,
};

#[derive(Debug, Serialize, Clone)]
pub struct ErrorMessage {
    code: u16,
    error_type: String,
    message: String,
    detail: String,
    field_name: String,
}

impl ErrorMessage {
    pub fn generic(code: StatusCode, message: &str, detail: String) -> Self {
        Self {
            code: code.as_u16(),
            error_type: "generic".to_string(),
            message: message.to_string(),
            detail,
            field_name: "".to_string(),
        }
    }

    pub fn validation(code: StatusCode, message: &str, field_name: String, detail: String) -> Self {
        Self {
            code: code.as_u16(),
            error_type: "validation".to_string(),
            message: message.to_string(),
            detail,
            field_name,
        }
    }
}

impl fmt::Display for ErrorMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_fmt(
            format_args!(
                "ErrorMessage(message: '{}', error_type: '{}', code: '{}', detail: '{}', field_name: '{}')",
                self.message, self.error_type, self.code, self.detail, self.field_name
            )
        )
    }
}

#[derive(Debug)]
struct LockErrorCause(String);

impl LockErrorCause {
    fn new<T>(error: PoisonError<T>, message: &str) -> Self {
        Self(format!("{} ({})", message, error))
    }
}

impl reject::Reject for LockErrorCause {}

/// Handler for start endpoint
pub async fn start_node_with_config(
    cfg: serde_json::Value,
    log: Logger,
    runner: LightNodeRunnerRef,
    client_runner: TezosClientRunnerRef,
    peers: Arc<Mutex<HashSet<NodeRpcIpPort>>>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to start the light node"; "config" => format!("{:?})", cfg));

    // aquire a write lock to the runner
    let mut runner = runner
        .write()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on runner"))?;

    // spawn the node
    info!(log, "Starting light-node...");
    let (node_ref, data_dir) = runner.spawn(cfg, &log)?;

    // initialize data for tezos client
    info!(log, "Initializing tezos-client data for light-node";
               "node_ref" => format!("{}", &node_ref),
               "client_data_dir" => data_dir.as_path().display().to_string());
    let mut client_runner = client_runner
        .write()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on client_runner"))?;
    client_runner.init_sandbox_data(node_ref.clone(), data_dir);

    // store node
    peers
        .lock()
        .map_err(|e| LockErrorCause::new(e, "Cannot get read lock on peers"))?
        .insert(node_ref.clone());

    info!(log, "Light-node started successfully!"; "node_ref" => format!("{}", &node_ref));
    Ok(warp::reply::with_status(
        warp::reply::json(&node_ref),
        StatusCode::OK,
    ))
}

pub async fn stop_node(
    log: Logger,
    runner: LightNodeRunnerRef,
    client_runner: TezosClientRunnerRef,
    peers: Arc<Mutex<HashSet<NodeRpcIpPort>>>,
    node_ref: Option<NodeRpcIpPort>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to stop the sandbox node..."; "node_ref" => format!("{:?}", &node_ref));

    let node_ref = ensure_node(node_ref)?;
    let mut runner = runner
        .write()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on runner"))?;
    let mut client_runner = client_runner
        .write()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on client_runner"))?;
    let mut errors = vec![];

    // try to stop sandbox node
    if let Err(e) = runner.shutdown(&node_ref) {
        errors.push(format!("{:?}", e));
    }

    // try to cleanup tezos client data
    if let Err(e) = client_runner.cleanup(&node_ref) {
        errors.push(format!("{:?}", e));
    }

    // remove from peers
    peers
        .lock()
        .map_err(|e| LockErrorCause::new(e, "Cannot get read lock on peers"))?
        .remove(&node_ref);

    if errors.is_empty() {
        info!(log, "Sandbox node stopped!"; "node_ref" => format!("{}", &node_ref));
        Ok(warp::reply::with_status(
            warp::reply::json(&""),
            StatusCode::OK,
        ))
    } else {
        error!(log, "Sandbox node stopped!";
                    "node_ref" => format!("{}", &node_ref),
                    "errors" => errors.join(", "));
        Ok(warp::reply::with_status(
            warp::reply::json(&errors),
            StatusCode::OK,
        ))
    }
}

pub async fn list_nodes(
    log: Logger,
    peers: Arc<Mutex<HashSet<NodeRpcIpPort>>>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to list sandbox nodes...");

    // return all nodes
    let mut nodes = peers
        .lock()
        .map_err(|e| LockErrorCause::new(e, "Cannot get read lock on peers"))?
        .iter()
        .cloned()
        .collect_vec();
    nodes.sort_by_key(|k| k.port);

    Ok(warp::reply::with_status(
        warp::reply::json(&nodes),
        StatusCode::OK,
    ))
}

pub async fn init_client_data(
    wallets: SandboxWallets,
    log: Logger,
    client_runner: TezosClientRunnerRef,
    node_ref: Option<NodeRpcIpPort>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to init the tezos-client");

    let node_ref = ensure_node(node_ref)?;
    let mut client_runner = client_runner
        .write()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on client_runner"))?;
    let client_output = client_runner.init_client_data(wallets, &node_ref, &log)?;

    reply_with_client_output(client_output, &log).map_err(|e| e.into())
}

pub async fn get_wallets(
    log: Logger,
    client_runner: TezosClientRunnerRef,
    node_ref: Option<NodeRpcIpPort>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to list the activated wallets");

    let node_ref = ensure_node(node_ref)?;
    let client_runner = client_runner
        .read()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on client_runner"))?;

    let wallets = client_runner
        .wallets(&node_ref)?
        .values()
        .cloned()
        .collect::<SandboxWallets>();
    let reply = warp::reply::json(&wallets);

    Ok(warp::reply::with_status(reply, StatusCode::OK))
}

pub async fn activate_protocol(
    activation_parameters: TezosProtcolActivationParameters,
    log: Logger,
    client_runner: TezosClientRunnerRef,
    node_ref: Option<NodeRpcIpPort>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to activate the protocol");

    let node_ref = ensure_node(node_ref)?;
    let client_runner = client_runner
        .read()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on client_runner"))?;
    let client_output = client_runner.activate_protocol(activation_parameters, &node_ref, &log)?;

    reply_with_client_output(client_output, &log).map_err(|e| e.into())
}

pub async fn bake_block_with_client(
    request: BakeRequest,
    log: Logger,
    client_runner: TezosClientRunnerRef,
    node_ref: Option<NodeRpcIpPort>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to bake a block");

    let node_ref = ensure_node(node_ref)?;
    let client_runner = client_runner
        .read()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on client_runner"))?;
    let client_output = client_runner.bake_block(Some(request), &node_ref, &log)?;

    reply_with_client_output(client_output, &log).map_err(|e| e.into())
}

pub async fn bake_block_with_client_arbitrary(
    log: Logger,
    client_runner: TezosClientRunnerRef,
    node_ref: Option<NodeRpcIpPort>,
) -> Result<impl warp::Reply, reject::Rejection> {
    info!(log, "Received request to bake arbitrary a block");

    let node_ref = ensure_node(node_ref)?;
    let client_runner = client_runner
        .read()
        .map_err(|e| LockErrorCause::new(e, "Cannot get write lock on client_runner"))?;
    let client_output = client_runner.bake_block(None, &node_ref, &log)?;

    reply_with_client_output(client_output, &log).map_err(|e| e.into())
}

pub async fn handle_rejection(err: Rejection, log: Logger) -> Result<impl Reply, Infallible> {
    let (code, error_message) = if err.is_not_found() {
        error!(log, "Rpc handle error"; "message" => "rpc not found");
        (
            StatusCode::NOT_FOUND,
            ErrorMessage::generic(StatusCode::NOT_FOUND, "rpc not found", "".to_string()),
        )
    } else if let Some(e) = err.find::<warp::filters::body::BodyDeserializeError>() {
        // This error happens if the body could not be deserialized correctly
        let detail = format!("{}", e);
        error!(log, "Rpc handle error"; "message" => "Request deserialization errror", "detail" => detail.clone());
        (
            StatusCode::BAD_REQUEST,
            ErrorMessage::generic(
                StatusCode::BAD_REQUEST,
                "Request deserialization errror",
                detail,
            ),
        )
    } else if let Some(tcre) = err.find::<TezosClientRunnerError>() {
        // Tezos client errors
        match tcre {
            TezosClientRunnerError::ProtocolParameterError { .. }
            | TezosClientRunnerError::NonexistantWallet { .. }
            | TezosClientRunnerError::UnavailableSandboxNodeError
            | TezosClientRunnerError::IOError { .. }
            | TezosClientRunnerError::SandboxDataDirNotInitialized { .. }
            | TezosClientRunnerError::SerdeError { .. } => {
                let message = format!("{}", tcre);
                error!(log, "Rpc handle error (tezos-client)"; "message" => message.clone());
                (
                    StatusCode::BAD_REQUEST,
                    ErrorMessage::generic(StatusCode::BAD_REQUEST, &message, "".to_string()),
                )
            }
            TezosClientRunnerError::CallError { message } => {
                error!(log, "Rpc handle error (tezos-client)"; "message" => format!("{:?}", message));
                (
                    StatusCode::from_u16(message.code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
                    message.clone(),
                )
            }
        }
    } else if let Some(lnre) = err.find::<LightNodeRunnerError>() {
        // Light-node errors
        match lnre {
            LightNodeRunnerError::JsonParsingError { .. }
            | LightNodeRunnerError::IOError { .. }
            | LightNodeRunnerError::ConfigurationMissingValidRpcPort { .. }
            | LightNodeRunnerError::NodeAlreadyRunning
            | LightNodeRunnerError::NodeNotRunning { .. } => {
                let message = format!("{}", lnre);
                error!(log, "Rpc handle error (light-node)"; "message" => message.clone());
                (
                    StatusCode::BAD_REQUEST,
                    ErrorMessage::generic(StatusCode::BAD_REQUEST, &message, "".to_string()),
                )
            }
            LightNodeRunnerError::NodeStartupError { reason } => match extract_field_name(reason) {
                Some(field_name) => {
                    let message = format!("{:?}", lnre);
                    error!(log, "Rpc handle error (light-node startup validation)"; "message" => message.clone(), "field_name" => field_name.clone());
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ErrorMessage::validation(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            reason,
                            field_name,
                            message,
                        ),
                    )
                }
                None => {
                    let message = format!("{:?}", lnre);
                    error!(log, "Rpc handle error (light-node startup)"; "message" => message.clone());
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        ErrorMessage::generic(StatusCode::INTERNAL_SERVER_ERROR, reason, message),
                    )
                }
            },
        }
    } else {
        let detail = format!("{:?}", err);
        error!(log, "Rpc handle error (light-node startup)"; "message" => "unhandled error occurred", "detail" => detail.clone());
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorMessage::generic(
                StatusCode::INTERNAL_SERVER_ERROR,
                "unhandled error occurred",
                detail,
            ),
        )
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&error_message),
        code,
    ))
}

fn extract_field_name(message: &str) -> Option<String> {
    let field_name = message
        .split_whitespace()
        .filter(|s| s.starts_with("\'--"))
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    if field_name.is_empty() {
        None
    } else {
        Some(field_name[0].replace("\'--", ""))
    }
}

/// Resolves which peer we want to call
pub fn resolve_node_from_request(
    peers: Arc<Mutex<HashSet<NodeRpcIpPort>>>,
) -> Option<NodeRpcIpPort> {
    // TODO: resolve some peer from request
    peers
        .lock()
        .unwrap() // TODO: reject when TE-213 is resolved, since now this should be infallible
        .iter()
        .next()
        .cloned()
}

fn ensure_node(node_ref: Option<NodeRpcIpPort>) -> Result<NodeRpcIpPort, TezosClientRunnerError> {
    match node_ref {
        Some(node_ref) => Ok(node_ref),
        None => Err(TezosClientRunnerError::UnavailableSandboxNodeError),
    }
}
