// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::path::{Path, PathBuf};

use clap::{App, Arg};

use tezos_api::environment::ZcashParams;

pub const DEFAULT_ZCASH_PARAM_SAPLING_SPEND_FILE_PATH: &str =
    "tezos/sys/lib_tezos/artifacts/sapling-spend.params";
pub const DEFAULT_ZCASH_PARAM_SAPLING_OUTPUT_FILE_PATH: &str =
    "tezos/sys/lib_tezos/artifacts/sapling-output.params";

pub struct LauncherEnvironment {
    pub light_node_path: PathBuf,
    pub protocol_runner_path: PathBuf,
    pub log_level: slog::Level,
    pub sandbox_rpc_port: u16,
    pub tezos_client_path: PathBuf,
    pub zcash_param: ZcashParams,
}

macro_rules! parse_validator_fn {
    ($t:ident, $err:expr) => {
        |v| {
            if v.parse::<$t>().is_ok() {
                Ok(())
            } else {
                Err($err.to_string())
            }
        }
    };
}

fn sandbox_app() -> App<'static, 'static> {
    let app = App::new("Tezos Light Node Launcher")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TezEdge and the project contributors")
        .setting(clap::AppSettings::AllArgsOverrideSelf)
        .arg(
            Arg::with_name("light-node-path")
                .long("light-node-path")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to the light-node binary")
                .required(true)
                .validator(|v| {
                    if Path::new(&v).exists() {
                        Ok(())
                    } else {
                        Err(format!("Light-node binary not found at '{}'", v))
                    }
                }),
        )
        .arg(
            Arg::with_name("protocol-runner-path")
                .long("protocol-runner-path")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to the protocol-runner binary")
                .required(true)
                .validator(|v| {
                    if Path::new(&v).exists() {
                        Ok(())
                    } else {
                        Err(format!("Protocol-runner binary not found at '{}'", v))
                    }
                }),
        )
        .arg(
            Arg::with_name("tezos-client-path")
                .long("tezos-client-path")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to the tezos-client binary")
                .required(true)
                .validator(|v| {
                    if Path::new(&v).exists() {
                        Ok(())
                    } else {
                        Err(format!("Tezos-client binary not found at '{}'", v))
                    }
                }),
        )
        .arg(
            Arg::with_name("log-level")
                .long("log-level")
                .takes_value(true)
                .value_name("LEVEL")
                .possible_values(&["critical", "error", "warn", "info", "debug", "trace"])
                .help("Set log level"),
        )
        .arg(
            Arg::with_name("sandbox-rpc-port")
                .long("sandbox-rpc-port")
                .takes_value(true)
                .value_name("PORT")
                .help("Rust server RPC port for communication with rust node")
                .required(true)
                .validator(parse_validator_fn!(
                    u16,
                    "Value must be a valid port number"
                )),
        )
        .arg(
            Arg::with_name("init-sapling-spend-params-file")
                .long("init-sapling-spend-params-file")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to a init file for sapling-spend.params"),
        )
        .arg(
            Arg::with_name("init-sapling-output-params-file")
                .long("init-sapling-output-params-file")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to a init file for sapling-output.params"),
        );

    app
}

impl LauncherEnvironment {
    pub fn from_args() -> Self {
        let app = sandbox_app();
        let args = app.clone().get_matches();

        LauncherEnvironment {
            light_node_path: args
                .value_of("light-node-path")
                .unwrap_or("")
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
            protocol_runner_path: args
                .value_of("protocol-runner-path")
                .unwrap_or("")
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
            log_level: args
                .value_of("log-level")
                .unwrap_or("")
                .parse::<slog::Level>()
                .expect("Was expecting one value from slog::Level"),
            sandbox_rpc_port: args
                .value_of("sandbox-rpc-port")
                .unwrap_or("")
                .parse::<u16>()
                .expect("Was expecting value of sandbox-rpc-port"),
            tezos_client_path: args
                .value_of("tezos-client-path")
                .unwrap_or("")
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
            zcash_param: ZcashParams {
                init_sapling_spend_params_file: args
                    .value_of("init-sapling-spend-params-file")
                    .unwrap_or(DEFAULT_ZCASH_PARAM_SAPLING_SPEND_FILE_PATH)
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
                init_sapling_output_params_file: args
                    .value_of("init-sapling-output-params-file")
                    .unwrap_or(DEFAULT_ZCASH_PARAM_SAPLING_OUTPUT_FILE_PATH)
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
            },
        }
    }
}
