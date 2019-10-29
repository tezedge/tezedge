pub mod encoding;
pub mod server;
pub mod rpc_actor;
mod helpers;
use chrono::prelude::*;

pub(crate) fn ts_to_rfc3339(ts: i64) -> String {
    Utc.from_utc_datetime(&NaiveDateTime::from_timestamp(ts, 0))
        .to_rfc3339_opts(SecondsFormat::Secs, true)
}