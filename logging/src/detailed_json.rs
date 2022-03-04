// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::io;

use slog::*;
use slog::{FnValue, Level, Record};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Get hostname for current machine
fn get_hostname() -> String {
    // Posix guarantees, that hostname might be up to 255 characters long (+1 for '\0')
    let mut buf = vec![0u8; 255];
    match nix::unistd::gethostname(&mut buf) {
        Ok(hostname_c) => hostname_c.to_string_lossy().into(),
        Err(_) => "n/a".to_string(),
    }
}

/// Convert logger level into integral value representing same level
///
/// # Arguments
/// * `level` - enum representation of logging level
fn level_to_int(level: Level) -> i8 {
    match level {
        Level::Critical => 60,
        Level::Error => 50,
        Level::Warning => 40,
        Level::Info => 30,
        Level::Debug => 20,
        Level::Trace => 10,
    }
}

/// Create new JSON logger with specific timestamp generator
///
/// # Arguments
/// * `io` - output writer to write logs
/// * `ts_f` - TimeStamp generator
fn new_with_ts_fn<F, W>(io: W, ts_f: F) -> slog_json::JsonBuilder<W>
where
    F: Fn(&Record) -> String + Send + Sync + std::panic::RefUnwindSafe + 'static,
    W: io::Write,
{
    slog_json::Json::new(io).add_key_value(o!(
        "pid" => nix::unistd::getpid().as_raw(),
        "hostname" => get_hostname(),
        "time" => FnValue(ts_f),
        "level" => FnValue(|rinfo : &Record| level_to_int(rinfo.level())),
        "module" => FnValue(|rinfo : &Record| rinfo.location().module.to_string()),
        "line" => FnValue(|rinfo : &Record| rinfo.location().line),
        "msg" => FnValue(|rinfo : &Record| rinfo.msg().to_string())
    ))
}

/// Create new default JSON logger
///
/// # Arguments
/// * `io` - output writer to write logs
pub fn default<W>(io: W) -> slog_json::Json<W>
where
    W: io::Write,
{
    const INVALID_TIME: &str = "invalid timestamp";
    new_with_ts_fn(io, |_: &Record| {
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_else(|_| String::from(INVALID_TIME))
    })
    .build()
}
