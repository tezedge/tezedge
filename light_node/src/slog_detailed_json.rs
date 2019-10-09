// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::io;

use chrono;
use nix;
use slog::*;
use slog::{FnValue, Level, Record};
use slog_json;

fn get_hostname() -> String {

    let mut buf = vec!(0u8; 256);
    match nix::unistd::gethostname(&mut buf) {
        Ok(hostname_c) => hostname_c.to_string_lossy().into(),
        Err(_) => "n/a".to_string(),
    }
}

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

fn new_with_ts_fn<F, W>(io : W, ts_f: F) -> slog_json::JsonBuilder<W>
    where F: Fn(&Record) -> String + Send + Sync + std::panic::RefUnwindSafe + 'static,
          W : io::Write
{
    slog_json::Json::new(io)
        .add_key_value(o!(
            "pid" => nix::unistd::getpid().as_raw(),
            "hostname" => get_hostname(),
            "time" => FnValue(ts_f),
            "level" => FnValue(|rinfo : &Record| level_to_int(rinfo.level())),
            "module" => FnValue(|rinfo : &Record| rinfo.location().module.to_string()),
            "line" => FnValue(|rinfo : &Record| rinfo.location().line),
            "msg" => FnValue(|rinfo : &Record| rinfo.msg().to_string())
        ))
}

pub fn default<W>(io : W) -> slog_json::Json<W>
    where
        W : io::Write {
    new_with_ts_fn(io, |_: &Record| chrono::Local::now().to_rfc3339()).build()
}

