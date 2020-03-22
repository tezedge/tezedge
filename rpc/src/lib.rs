// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use chrono::prelude::*;
use hyper::{Body, Response, StatusCode};
use slog::{Logger, warn};

use crypto::hash::HashType;
pub use storage::persistent::{ContextList, ContextMap};

use crate::rpc_actor::RpcCollectedStateRef;

pub mod encoding;
mod helpers;
pub mod rpc_actor;
mod server;

/// Crate level custom result
pub(crate) type ServiceResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

/// Helper function to format UNIX (integral) timestamp to RFC3339 string timestamp
pub(crate) fn ts_to_rfc3339(ts: i64) -> String {
    Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Function to generate JSON response from serializable object
pub(crate) fn make_json_response<T: serde::Serialize>(content: &T) -> ServiceResult {
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/json")
        // TODO: add to config
        .header(hyper::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(serde_json::to_string(content)?))?)
}

/// Returns result as a JSON response.
pub(crate) fn result_to_json_response<T: serde::Serialize>(res: Result<T, failure::Error>, log: &Logger) -> ServiceResult {
    match res {
        Ok(t) => make_json_response(&t),
        Err(err) => {
            warn!(log, "Failed to execute RPC function"; "reason" => format!("{:?}", err));
            empty()
        }
    }
}

/// Returns optional result as a JSON response.
pub(crate) fn result_option_to_json_response<T: serde::Serialize>(res: Result<Option<T>, failure::Error>, log: &Logger) -> ServiceResult {
    match res {
        Ok(opt) => match opt {
            Some(t) => make_json_response(&t),
            None => not_found()
        }
        Err(err) => {
            warn!(log, "Failed to execute RPC function"; "reason" => format!("{:?}", err));
            empty()
        }
    }
}

/// Generate empty response
pub(crate) fn empty() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(204)?)
        .body(Body::empty())?)
}

/// Unwraps a block hash or provides alternative block hash.
/// Alternatives are: genesis block or current head
pub(crate) fn unwrap_block_hash(block_id: Option<&str>, state: &RpcCollectedStateRef, genesis_hash: &str) -> String {
    block_id.map(String::from).unwrap_or_else(|| {
        let state = state.read().unwrap();
        state.current_head().as_ref()
            .map(|current_head| HashType::BlockHash.bytes_to_string(&current_head.header().hash))
            .unwrap_or(genesis_hash.to_string())
    })
}

/// Generate 404 response
pub(crate) fn not_found() -> ServiceResult {
    Ok(Response::builder()
        .status(StatusCode::from_u16(404)?)
        .body(Body::from("not found"))?)
}