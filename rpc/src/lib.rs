pub mod encoding;
pub mod server;
pub mod rpc_actor;
mod helpers;

use chrono::prelude::*;
use hyper::{Response, Body};

pub(crate) type ServiceResult = Result<Response<Body>, Box<dyn std::error::Error + Sync + Send>>;

pub(crate) fn ts_to_rfc3339(ts: i64) -> String {
    Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}

pub(crate) fn make_json_response<T: serde::Serialize>(content: &T) -> ServiceResult {
    Ok(Response::builder()
        .header(hyper::header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_string(content)?))?)
}