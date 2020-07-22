// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{env, thread};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use serde::{Deserialize, Serialize};
use slog::{Drain, Level, Logger};

pub fn prepare_empty_dir(dir_name: &str) -> String {
    let path = test_storage_dir_path(dir_name);
    if path.exists() {
        fs::remove_dir_all(&path).unwrap_or_else(|_| panic!("Failed to delete directory: {:?}", &path));
    }
    fs::create_dir_all(&path).unwrap_or_else(|_| panic!("Failed to create directory: {:?}", &path));
    String::from(path.to_str().unwrap())
}

pub fn test_storage_dir_path(dir_name: &str) -> PathBuf {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let path = Path::new(out_dir.as_str())
        .join(Path::new(dir_name))
        .to_path_buf();
    path
}

pub fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(
            slog_term::TermDecorator::new().build()
        ).build().fuse()
    ).build().filter_level(level).fuse();

    Logger::root(drain, slog::o!())
}

pub fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>().unwrap()
}

pub fn no_of_ffi_calls_treshold_for_gc() -> i32 {
    env::var("OCAML_CALLS_GC")
        .unwrap_or("2000".to_string())
        .parse::<i32>().unwrap()
}

pub fn log_level() -> Level {
    env::var("LOG_LEVEL")
        .unwrap_or("info".to_string())
        .parse::<Level>().unwrap()
}

pub fn protocol_runner_executable_path() -> PathBuf {
    let executable = env::var("PROTOCOL_RUNNER")
        .unwrap_or_else(|_| panic!("This test requires environment parameter: 'PROTOCOL_RUNNER' to point to protocol_runner executable"));
    PathBuf::from(executable)
}

/// Empty message
#[derive(Serialize, Deserialize, Debug)]
pub struct NoopMessage;