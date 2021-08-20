// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use hyper::{Body, Response, StatusCode};
use slog::{error, Logger};

use crate::helpers::RpcServiceError;

pub mod encoding;
mod helpers;
mod services;

mod server;
pub use server::rpc_server::RpcServer;

/// Crate level custom result
pub type ServiceResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

/// Generate options response with supported methods, headers
pub(crate) fn options() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(200)?)
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .body(Body::empty())?)
}

/// Function to generate JSON response from serializable object
pub fn make_json_response<T: serde::Serialize>(content: &T) -> ServiceResult {
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/json")
        // TODO: add to config
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .body(Body::from(serde_json::to_string(content)?))?)
}

pub fn make_raw_response(raw: &'static [u8]) -> ServiceResult {
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/json")
        // TODO: add to config
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .body(Body::from(raw))?)
}

/// Produces a JSON response from an FFI RPC response
pub fn make_response_with_status_and_json_string(status_code: u16, body: &str) -> ServiceResult {
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/json")
        // TODO: add to config
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .status(status_code)
        .body(Body::from(body.to_owned()))?)
}

/// Function to generate JSON response from a stream
pub(crate) fn make_json_stream_response<
    T: futures::Stream<Item = Result<String, failure::Error>> + Send + 'static,
>(
    content: T,
) -> ServiceResult {
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(
            hyper::header::ACCESS_CONTROL_ALLOW_METHODS,
            "GET, POST, OPTIONS, PUT",
        )
        .body(Body::wrap_stream(content))?)
}

/// Returns result as a JSON response.
pub(crate) fn result_to_json_response<T: serde::Serialize>(
    res: Result<T, RpcServiceError>,
    log: &Logger,
) -> ServiceResult {
    match res {
        Ok(t) => make_json_response(&t),
        Err(err) => {
            error!(log, "Failed to execute RPC function"; "reason" => format!("{:?}", &err));
            handle_rpc_service_error(err)
        }
    }
}

/// Returns optional result as a JSON response.
pub(crate) fn result_option_to_json_response<T: serde::Serialize>(
    res: Result<Option<T>, RpcServiceError>,
    log: &Logger,
) -> ServiceResult {
    match res {
        Ok(opt) => match opt {
            Some(t) => make_json_response(&t),
            None => not_found(),
        },
        Err(err) => {
            error!(log, "Failed to execute RPC function"; "reason" => format!("{:?}", &err));
            handle_rpc_service_error(err)
        }
    }
}

/// Returns result as a empty JSON response: `{}`.
pub(crate) fn result_to_empty_json_response(
    res: Result<(), RpcServiceError>,
    log: &Logger,
) -> ServiceResult {
    match res {
        Ok(_) => {
            let empty_json = serde_json::json!({});
            make_json_response(&empty_json)
        }
        Err(err) => {
            error!(log, "Failed to execute RPC function"; "reason" => format!("{:?}", &err));
            handle_rpc_service_error(err)
        }
    }
}

/// Generate empty response
pub(crate) fn empty() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(204)?)
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .body(Body::empty())?)
}

/// Generate 404 response
pub(crate) fn not_found() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(404)?)
        .header(hyper::header::CONTENT_TYPE, "text/plain")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .body(Body::empty())?)
}

/// Generate 500 error
pub(crate) fn error(error: failure::Error) -> ServiceResult {
    error_with_message(format!("{:?}", error))
}

pub(crate) fn handle_rpc_service_error(error: RpcServiceError) -> ServiceResult {
    match error {
        RpcServiceError::StorageError { error } => error_with_message(format!("{:?}", error)),
        RpcServiceError::InvalidParameters { reason } => error_with_message(reason),
        RpcServiceError::UnexpectedError { reason } => error_with_message(reason),
        RpcServiceError::NoDataFoundError { .. } => not_found(),
    }
}

/// Generate 500 error with message as body
pub(crate) fn error_with_message(error_msg: String) -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(500)?)
        .header(hyper::header::CONTENT_TYPE, "text/plain")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .header(hyper::header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type")
        .header(hyper::header::TRANSFER_ENCODING, "chunked")
        .body(Body::from(error_msg))?)
}
