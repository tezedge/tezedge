pub mod encoding;
mod helpers;
pub mod rpc_actor;
pub mod server;

use chrono::prelude::*;
use hyper::{Body, Response};

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
