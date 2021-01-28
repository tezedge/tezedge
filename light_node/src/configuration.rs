// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, BufRead};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{App, Arg};

use shell::peer_manager::P2p;
use shell::PeerConnectionThreshold;
use storage::persistent::{DbConfiguration, DbConfigurationBuilder};
use tezos_api::environment;
use tezos_api::environment::TezosEnvironment;
use tezos_api::ffi::PatchContext;
use tezos_wrapper::TezosApiConnectionPoolConfiguration;

#[derive(Debug, Clone)]
pub struct Rpc {
    pub listener_port: u16,
    pub websocket_address: SocketAddr,
}

#[derive(Debug, Clone)]
pub struct Logging {
    pub ocaml_log_enabled: bool,
    pub level: slog::Level,
    pub format: LogFormat,
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Storage {
    pub db_cfg: DbConfiguration,
    pub db_path: PathBuf,
    pub tezos_data_dir: PathBuf,
    pub store_context_actions: bool,
    pub patch_context: Option<PatchContext>,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub identity_json_file_path: PathBuf,
    pub expected_pow: f64,
}

#[derive(Debug, Clone)]
pub struct Ffi {
    pub protocol_runner: PathBuf,
    pub no_of_ffi_calls_threshold_for_gc: i32,
    pub tezos_readonly_api_pool: TezosApiConnectionPoolConfiguration,
    pub tezos_readonly_prevalidation_api_pool: TezosApiConnectionPoolConfiguration,
    pub tezos_without_context_api_pool: TezosApiConnectionPoolConfiguration,
}

impl Ffi {
    const TEZOS_READONLY_API_POOL_DISCRIMINATOR: &'static str = "";
    const TEZOS_READONLY_PREVALIDATION_API_POOL_DISCRIMINATOR: &'static str = "trpap";
    const TEZOS_WITHOUT_CONTEXT_API_POOL_DISCRIMINATOR: &'static str = "twcap";
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Json,
    Simple,
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "simple" => Ok(LogFormat::Simple),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!("Unsupported variant: {}", s)),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub p2p: P2p,
    pub rpc: Rpc,
    pub logging: Logging,
    pub storage: Storage,
    pub identity: Identity,
    pub ffi: Ffi,

    pub tezos_network: TezosEnvironment,
    pub enable_testchain: bool,
    pub tokio_threads: usize,

    /// This flag is used, just for to stop node immediatelly after generate identity,
    /// to prevent and initialize actors and create data (except identity)
    pub validate_cfg_identity_and_stop: bool,
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

// Creates tezos app
pub fn tezos_app() -> App<'static, 'static> {
    // Default values for arguments are specidied in default configuration file
    //
    // Flag Required=true must be handled separately as we parse args twice,
    // once to see only if confi-file arg is present and second time to parse all args
    //
    // In case some args are required=true and user provides only config-file,
    // first round of parsing would always fail then
    let app = App::new("Tezos Light Node")
        .version("0.3.1")
        .author("SimpleStaking and the project contributors")
        .about("Rust implementation of the tezos node")
        .setting(clap::AppSettings::AllArgsOverrideSelf)
        .arg(Arg::with_name("validate-cfg-identity-and-stop")
            .long("validate-cfg-identity-and-stop")
            .takes_value(false)
            .help("Validate configuration and generated identity, than just stops application"))
        .arg(Arg::with_name("config-file")
            .long("config-file")
            .takes_value(true)
            .value_name("PATH")
            .help("Configuration file with start-up arguments (same format as cli arguments)")
            .validator(|v| if Path::new(&v).exists() { Ok(()) } else { Err(format!("Configuration file not found at '{}'", v)) }))
        .arg(Arg::with_name("tezos-data-dir")
            .long("tezos-data-dir")
            .takes_value(true)
            .value_name("PATH")
            .help("A directory for Tezos OCaml runtime storage (context/store)")
            .validator(|v| {
                let dir = Path::new(&v);
                if dir.exists() {
                    if dir.is_dir() {
                        Ok(())
                    } else {
                        Err(format!("Required tezos data dir '{}' exists, but is not a directory!", v))
                    }
                } else {
                    // Tezos data dir does not exists, try to create it
                    if let Err(e) = fs::create_dir_all(dir) {
                        Err(format!("Unable to create required tezos data dir '{}': {} ", v, e))
                    } else {
                        Ok(())
                    }
                }
            }))
        .arg(Arg::with_name("identity-file")
            .long("identity-file")
            .takes_value(true)
            .value_name("PATH")
            .help("Path to the json identity file with peer-id, public-key, secret-key and pow-stamp.
                       In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir"))
        .arg(Arg::with_name("identity-expected-pow")
            .long("identity-expected-pow")
            .takes_value(true)
            .value_name("NUM")
            .help("Expected power of identity for node. It is used to generate new identity. Default: 26.0")
            .validator(parse_validator_fn!(f64, "Value must be a valid f64 number for expected_pow")))
        .arg(Arg::with_name("bootstrap-db-path")
            .long("bootstrap-db-path")
            .takes_value(true)
            .value_name("PATH")
            .help("Path to bootstrap database directory.
                       In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir"))
        .arg(Arg::with_name("db-cfg-max-threads")
            .long("db-cfg-max-threads")
            .takes_value(true)
            .value_name("NUM")
            .help("Max number of threads used by database configuration. If not specified, then number of threads equal to CPU cores.")
            .validator(parse_validator_fn!(usize, "Value must be a valid number")))
        .arg(Arg::with_name("bootstrap-lookup-address")
            .long("bootstrap-lookup-address")
            .takes_value(true)
            .conflicts_with("peers")
            .conflicts_with("private-node")
            .help("A peers for dns lookup to get the peers to bootstrap the network from. Peers are delimited by a colon. Default: used according to --network parameter see TezosEnvironment"))
        .arg(Arg::with_name("disable-bootstrap-lookup")
            .long("disable-bootstrap-lookup")
            .takes_value(false)
            .conflicts_with("bootstrap-lookup-address")
            .help("Disables dns lookup to get the peers to bootstrap the network from. Default: false"))
        .arg(Arg::with_name("log-file")
            .long("log-file")
            .takes_value(true)
            .value_name("PATH")
            .help("Path to the log file. If provided, logs are displayed the log file, otherwise in terminal.
                       In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir"))
        .arg(Arg::with_name("log-format")
            .long("log-format")
            .takes_value(true)
            .possible_values(&["json", "simple"])
            .help("Set output format of the log"))
        .arg(Arg::with_name("log-level")
            .long("log-level")
            .takes_value(true)
            .value_name("LEVEL")
            .possible_values(&["critical", "error", "warn", "info", "debug", "trace"])
            .help("Set log level"))
        .arg(Arg::with_name("ocaml-log-enabled")
            .long("ocaml-log-enabled")
            .takes_value(true)
            .value_name("BOOL")
            .help("Flag for turn on/off logging in Tezos OCaml runtime"))
        .arg(Arg::with_name("disable-mempool")
            .long("disable-mempool")
            .help("Enable or disable mempool"))
        .arg(Arg::with_name("private-node")
            .long("private-node")
            .takes_value(true)
            .value_name("BOOL")
            .requires("peers")
            .conflicts_with("bootstrap-lookup-address")
            .help("Enable or disable private node. Use peers to set IP addresses of the peers you want to connect to"))
        .arg(Arg::with_name("network")
            .long("network")
            .takes_value(true)
            .possible_values(&["alphanet", "babylonnet", "babylon", "mainnet", "zeronet", "carthagenet", "carthage", "delphinet", "delphi", "edonet", "edo", "sandbox"])
            .help("Choose the Tezos environment"))
        .arg(Arg::with_name("p2p-port")
            .long("p2p-port")
            .takes_value(true)
            .value_name("PORT")
            .help("Socket listening port for p2p for communication with tezos world")
            .validator(parse_validator_fn!(u16, "Value must be a valid port number")))
        .arg(Arg::with_name("rpc-port")
            .long("rpc-port")
            .takes_value(true)
            .value_name("PORT")
            .help("Rust server RPC port for communication with rust node")
            .validator(parse_validator_fn!(u16, "Value must be a valid port number")))
        .arg(Arg::with_name("enable-testchain")
            .long("enable-testchain")
            .takes_value(true)
            .value_name("BOOL")
            .help("Flag for enable/disable test chain switching for block applying. Default: false"))
        .arg(Arg::with_name("websocket-address")
            .long("websocket-address")
            .takes_value(true)
            .value_name("IP:PORT")
            .help("Websocket address where various node metrics and statistics are available")
            .validator(parse_validator_fn!(SocketAddr, "Value must be a valid IP:PORT")))
        .arg(Arg::with_name("peers")
            .long("peers")
            .takes_value(true)
            .value_name("IP:PORT")
            .help("A peer to bootstrap the network from. Peers are delimited by a colon. Format: IP1:PORT1,IP2:PORT2,IP3:PORT3")
            .validator(|v| {
                let err_count = v.split(',')
                    .map(|ip_port| ip_port.parse::<SocketAddr>())
                    .filter(|v| v.is_err())
                    .count();
                if err_count == 0 {
                    Ok(())
                } else {
                    Err(format!("Value '{}' is not valid. Expected format is: IP1:PORT1,IP2:PORT2,IP3:PORT3", v))
                }
            }))
        .arg(Arg::with_name("peer-thresh-low")
            .long("peer-thresh-low")
            .takes_value(true)
            .value_name("NUM")
            .help("Minimal number of peers to connect to")
            .validator(parse_validator_fn!(usize, "Value must be a valid number")))
        .arg(Arg::with_name("peer-thresh-high")
            .long("peer-thresh-high")
            .takes_value(true)
            .value_name("NUM")
            .help("Maximal number of peers to connect to")
            .validator(parse_validator_fn!(usize, "Value must be a valid number")))
        .arg(Arg::with_name("synchronization-thresh")
            .long("synchronization-thresh")
            .takes_value(true)
            .value_name("NUM")
            .help("Maximal number of peers to connect to")
            .validator(parse_validator_fn!(usize, "Value must be a valid number")))
        .arg(Arg::with_name("protocol-runner")
            .long("protocol-runner")
            .takes_value(true)
            .value_name("PATH")
            .help("Path to a tezos protocol runner executable")
            .validator(|v| if Path::new(&v).exists() { Ok(()) } else { Err(format!("Tezos protocol runner executable not found at '{}'", v)) }))
        .arg(Arg::with_name("ffi-calls-gc-threshold")
            .long("ffi-calls-gc-threshold")
            .takes_value(true)
            .value_name("NUM")
            .help("Number of ffi calls, after which will be Ocaml garbage collector called")
            .validator(parse_validator_fn!(i32, "Value must be a valid number")))
        .args(
            &[
                Arg::with_name("ffi-pool-max-connections")
                    .long("ffi-pool-max-connections")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of max ffi pool connections, default: 10")
                    .validator(parse_validator_fn!(u8, "Value must be a valid number")),
                Arg::with_name("ffi-pool-connection-timeout-in-secs")
                    .long("ffi-pool-connection-timeout-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to wait for connection, default: 60")
                    .validator(parse_validator_fn!(u16, "Value must be a valid number")),
                Arg::with_name("ffi-pool-max-lifetime-in-secs")
                    .long("ffi-pool-max-lifetime-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to remove protocol_runner from pool, default: 21600 means 6 hours")
                    .validator(parse_validator_fn!(u64, "Value must be a valid number")),
                Arg::with_name("ffi-pool-idle-timeout-in-secs")
                    .long("ffi-pool-idle-timeout-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to remove unused protocol_runner from pool, default: 1800 means 30 minutes")
                    .validator(parse_validator_fn!(u64, "Value must be a valid number"))
            ])
        .args(
            &[
                Arg::with_name("ffi-trpap-pool-max-connections")
                    .long("ffi-trpap-pool-max-connections")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of max ffi pool connections, default: 10")
                    .validator(parse_validator_fn!(u8, "Value must be a valid number")),
                Arg::with_name("ffi-trpap-pool-connection-timeout-in-secs")
                    .long("ffi-trpap-pool-connection-timeout-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to wait for connection, default: 60")
                    .validator(parse_validator_fn!(u16, "Value must be a valid number")),
                Arg::with_name("ffi-trpap-pool-max-lifetime-in-secs")
                    .long("ffi-trpap-pool-max-lifetime-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to remove protocol_runner from pool, default: 21600 means 6 hours")
                    .validator(parse_validator_fn!(u64, "Value must be a valid number")),
                Arg::with_name("ffi-trpap-pool-idle-timeout-in-secs")
                    .long("ffi-trpap-pool-idle-timeout-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to remove unused protocol_runner from pool, default: 1800 means 30 minutes")
                    .validator(parse_validator_fn!(u64, "Value must be a valid number"))
            ])
        .args(
            &[
                Arg::with_name("ffi-twcap-pool-max-connections")
                    .long("ffi-twcap-pool-max-connections")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of max ffi pool connections, default: 10")
                    .validator(parse_validator_fn!(u8, "Value must be a valid number")),
                Arg::with_name("ffi-twcap-pool-connection-timeout-in-secs")
                    .long("ffi-twcap-pool-connection-timeout-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to wait for connection, default: 60")
                    .validator(parse_validator_fn!(u16, "Value must be a valid number")),
                Arg::with_name("ffi-twcap-pool-max-lifetime-in-secs")
                    .long("ffi-twcap-pool-max-lifetime-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to remove protocol_runner from pool, default: 21600 means 6 hours")
                    .validator(parse_validator_fn!(u64, "Value must be a valid number")),
                Arg::with_name("ffi-twcap-pool-idle-timeout-in-secs")
                    .long("ffi-twcap-pool-idle-timeout-in-secs")
                    .takes_value(true)
                    .value_name("NUM")
                    .help("Number of seconds to remove unused protocol_runner from pool, default: 1800 means 30 minutes")
                    .validator(parse_validator_fn!(u64, "Value must be a valid number"))
            ])
        .arg(Arg::with_name("tokio-threads")
            .long("tokio-threads")
            .takes_value(true)
            .value_name("NUM")
            .help("Number of threads spawned by a tokio thread pool. If value is zero, then number of threads equal to CPU cores is spawned.")
            .validator(parse_validator_fn!(usize, "Value must be a valid number")))
        .arg(Arg::with_name("store-context-actions")
            .long("store-context-actions")
            .takes_value(true)
            .value_name("BOOL")
            .help("Activate recording of context storage actions"))
        .arg(Arg::with_name("sandbox-patch-context-json-file")
            .long("sandbox-patch-context-json-file")
            .takes_value(true)
            .value_name("PATH")
            .required(false)
            .help("Path to the json file with key-values, which will be added to empty context on startup and commit genesis.")
            .validator(|v| if Path::new(&v).exists() { Ok(()) } else { Err(format!("Sandbox patch-context json file not found at '{}'", v)) }));
    app
}

fn pool_cfg(
    args: &clap::ArgMatches,
    pool_name_discriminator: &str,
) -> TezosApiConnectionPoolConfiguration {
    TezosApiConnectionPoolConfiguration {
        min_connections: 0,
        /* 0 means that connections are created on-demand, because of AT_LEAST_ONE_WRITE_PROTOCOL_CONTEXT_WAS_SUCCESS_AT_FIRST_LOCK */
        max_connections: args
            .value_of(&format!(
                "ffi-{}-pool-max-connections",
                pool_name_discriminator
            ))
            .unwrap_or("10")
            .parse::<u8>()
            .expect("Provided value cannot be converted to number"),
        connection_timeout: args
            .value_of(&format!(
                "ffi-{}-pool-connection-timeout-in-secs",
                pool_name_discriminator
            ))
            .unwrap_or("60")
            .parse::<u16>()
            .map(|seconds| Duration::from_secs(seconds as u64))
            .expect("Provided value cannot be converted to number"),
        max_lifetime: args
            .value_of(&format!(
                "ffi-{}-pool-max-lifetime-in-secs",
                pool_name_discriminator
            ))
            .unwrap_or("21600")
            .parse::<u16>()
            .map(|seconds| Duration::from_secs(seconds as u64))
            .expect("Provided value cannot be converted to number"),
        idle_timeout: args
            .value_of(&format!(
                "ffi-{}-pool-idle-timeout-in-secs",
                pool_name_discriminator
            ))
            .unwrap_or("1800")
            .parse::<u16>()
            .map(|seconds| Duration::from_secs(seconds as u64))
            .expect("Provided value cannot be converted to number"),
    }
}

// Explicitly validates all required parameters
// Flag Required=true must be handled separately as we parse args twice,
// once to see only if config-file arg is present and second time to parse all args
// In case some args are required=true and user provides only config-file,
// first round of parsing would always fail then
fn validate_required_args(args: &clap::ArgMatches) {
    validate_required_arg(args, "tezos-data-dir");
    validate_required_arg(args, "network");
    validate_required_arg(args, "bootstrap-db-path");
    validate_required_arg(args, "log-format");
    validate_required_arg(args, "ocaml-log-enabled");
    validate_required_arg(args, "p2p-port");
    validate_required_arg(args, "protocol-runner");
    validate_required_arg(args, "rpc-port");
    validate_required_arg(args, "websocket-address");
    validate_required_arg(args, "peer-thresh-low");
    validate_required_arg(args, "peer-thresh-high");
    validate_required_arg(args, "ffi-calls-gc-threshold");
    validate_required_arg(args, "tokio-threads");
    validate_required_arg(args, "identity-file");
    validate_required_arg(args, "identity-expected-pow");

    // "bootstrap-lookup-address", "log-file" and "peers" are not required
}

// Validates single required arg. If missing, exit whole process
pub fn validate_required_arg(args: &clap::ArgMatches, arg_name: &str) {
    if !args.is_present(arg_name) {
        panic!("required \"{}\" arg is missing !!!", arg_name);
    }
}

// Returns final path. In case:
//      1. path is relative -> final_path = tezos_data_dir / path
//      2. path is absolute -> final_path = path
pub fn get_final_path(tezos_data_dir: &Path, path: PathBuf) -> PathBuf {
    let mut final_path: PathBuf;

    // path is absolute or relative to the current dir -> start with ./ or ../
    if path.is_absolute() || path.starts_with(".") {
        final_path = path
    }
    // otherwise path is relative to the tezos-data-dir
    else {
        final_path = tezos_data_dir.to_path_buf();
        final_path.push(path);
    }

    // Tries to create final_path parent dir, if non-existing
    if let Some(parent_dir) = final_path.parent() {
        if !parent_dir.exists() {
            if let Err(e) = fs::create_dir_all(parent_dir) {
                panic!("Unable to create required dir '{:?}': {} ", parent_dir, e);
            }
        }
    }

    final_path
}

// Parses config file and returns vector of OsString representing all argument strings from file
// All lines that are empty or begin with "#" or "//" are ignored
pub fn parse_config(config_path: PathBuf) -> Vec<OsString> {
    let file = fs::File::open(&config_path)
        .unwrap_or_else(|_| panic!("Unable to open config file at: {:?}", config_path));
    let reader = io::BufReader::new(file);

    let mut args: Vec<OsString> = vec![];

    let mut line_num = 0;
    for line_result in reader.lines() {
        let mut line = line_result.unwrap_or_else(|_| {
            panic!(
                "Unable to read line: {:?} from config file at: {:?}",
                line_num, config_path
            )
        });
        line = line.trim().to_string();

        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }

        args.push(OsString::from(line));
        line_num += 1;
    }

    args
}

impl Environment {
    pub fn from_args() -> Self {
        let app = tezos_app();
        let args: clap::ArgMatches;

        // First, get cli arguments and find out only if config-file arg is provided
        // If config-file argument is present, read all parameters from config-file and merge it with cli arguments
        let temp_args = app.clone().get_matches();
        if temp_args.is_present("config-file") {
            let config_path = temp_args
                .value_of("config-file")
                .unwrap()
                .parse::<PathBuf>()
                .expect("Provided config-file cannot be converted to path");

            let mut merged_args = parse_config(config_path);

            let mut cli_args = env::args_os();
            if let Some(bin) = cli_args.next() {
                merged_args.insert(0, bin);
            }
            merged_args.extend(cli_args);

            args = app.get_matches_from(merged_args);
        }
        // Otherwise use only cli arguments that are already parsed
        else {
            args = temp_args;
        }

        // Validates required flags of args
        validate_required_args(&args);

        let tezos_network: TezosEnvironment = args
            .value_of("network")
            .unwrap_or("")
            .parse::<TezosEnvironment>()
            .expect("Was expecting one value from TezosEnvironment");

        let data_dir: PathBuf = args
            .value_of("tezos-data-dir")
            .unwrap_or("")
            .parse::<PathBuf>()
            .expect("Provided value cannot be converted to path");

        Environment {
            p2p: crate::configuration::P2p {
                listener_port: args
                    .value_of("p2p-port")
                    .unwrap_or("")
                    .parse::<u16>()
                    .expect("Was expecting value of p2p-port"),
                disable_bootstrap_lookup: args.is_present("disable-bootstrap-lookup"),
                bootstrap_lookup_addresses: args
                    .value_of("bootstrap-lookup-address")
                    .map(|addresses_str| {
                        addresses_str
                            .split(',')
                            .map(|address| address.to_string())
                            .collect()
                    })
                    .unwrap_or_else(|| {
                        if !args.is_present("peers") && !args.is_present("private-node") {
                            match environment::TEZOS_ENV.get(&tezos_network) {
                                None => panic!(
                                    "No tezos environment configured for: {:?}",
                                    tezos_network
                                ),
                                Some(cfg) => cfg.bootstrap_lookup_addresses.clone(),
                            }
                        } else {
                            Vec::with_capacity(0)
                        }
                    })
                    .iter()
                    .map(|addr| {
                        (
                            addr.clone(),
                            crate::configuration::P2p::DEFAULT_P2P_PORT_FOR_LOOKUP,
                        )
                    })
                    .collect(),
                bootstrap_peers: args
                    .value_of("peers")
                    .map(|peers_str| {
                        peers_str
                            .split(',')
                            .map(|ip_port| ip_port.parse().expect("Was expecting IP:PORT"))
                            .collect()
                    })
                    .unwrap_or_default(),
                peer_threshold: PeerConnectionThreshold::new(
                    args.value_of("peer-thresh-low")
                        .unwrap_or("")
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                    args.value_of("peer-thresh-high")
                        .unwrap_or("")
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                    args.value_of("synchronization-thresh").map(|v| {
                        v.parse::<usize>()
                            .expect("Provided value cannot be converted to number")
                    }),
                ),
                private_node: args
                    .value_of("private-node")
                    .unwrap_or("false")
                    .parse::<bool>()
                    .expect("Provided value cannot be converted to bool"),
                disable_mempool: args.is_present("disable-mempool"),
            },
            rpc: crate::configuration::Rpc {
                listener_port: args
                    .value_of("rpc-port")
                    .unwrap_or("")
                    .parse::<u16>()
                    .expect("Was expecting value of rpc-port"),
                websocket_address: args
                    .value_of("websocket-address")
                    .unwrap_or("")
                    .parse()
                    .expect("Provided value cannot be converted into valid uri"),
            },
            logging: crate::configuration::Logging {
                ocaml_log_enabled: args
                    .value_of("ocaml-log-enabled")
                    .unwrap_or("")
                    .parse::<bool>()
                    .expect("Provided value cannot be converted to bool"),
                level: args
                    .value_of("log-level")
                    .unwrap_or("")
                    .parse::<slog::Level>()
                    .expect("Was expecting one value from slog::Level"),
                format: args
                    .value_of("log-format")
                    .unwrap_or("")
                    .parse::<LogFormat>()
                    .expect("Was expecting 'simple' or 'json'"),
                file: {
                    let log_file_path = args.value_of("log-file").map(|v| {
                        v.parse::<PathBuf>()
                            .expect("Provided value cannot be converted to path")
                    });

                    if let Some(path) = log_file_path {
                        Some(get_final_path(&data_dir, path))
                    } else {
                        log_file_path
                    }
                },
            },
            storage: crate::configuration::Storage {
                tezos_data_dir: data_dir.clone(),
                db_cfg: {
                    let mut db_cfg = DbConfigurationBuilder::default();

                    if let Some(value) = args.value_of("db-cfg-max-threads") {
                        let max_treads = value
                            .parse::<usize>()
                            .expect("Provided value cannot be converted to number");
                        db_cfg.max_threads(Some(max_treads));
                    }

                    db_cfg.build().unwrap()
                },
                db_path: {
                    let db_path = args
                        .value_of("bootstrap-db-path")
                        .unwrap_or("")
                        .parse::<PathBuf>()
                        .expect("Provided value cannot be converted to path");
                    get_final_path(&data_dir, db_path)
                },
                store_context_actions: args
                    .value_of("store-context-actions")
                    .unwrap_or("true")
                    .parse::<bool>()
                    .expect("Provided value cannot be converted to bool"),
                patch_context: {
                    match args.value_of("sandbox-patch-context-json-file") {
                        Some(path) => {
                            let path = path
                                .parse::<PathBuf>()
                                .expect("Provided value cannot be converted to path");
                            let path = get_final_path(&data_dir, path);
                            match fs::read_to_string(&path) {
                                Ok(content) => {
                                    // validate valid json
                                    if let Err(e) = serde_json::from_str::<
                                        HashMap<String, serde_json::Value>,
                                    >(&content)
                                    {
                                        panic!(
                                            "Invalid json file: {}, reason: {}",
                                            path.as_path().display().to_string(),
                                            e
                                        );
                                    }
                                    Some(PatchContext {
                                        key: "sandbox_parameter".to_string(),
                                        json: content,
                                    })
                                }
                                Err(e) => panic!("Cannot read file, reason: {}", e),
                            }
                        }
                        None => {
                            // check default configuration, if any
                            match environment::TEZOS_ENV.get(&tezos_network) {
                                None => panic!(
                                    "No tezos environment configured for: {:?}",
                                    tezos_network
                                ),
                                Some(cfg) => cfg.patch_context_genesis_parameters.clone(),
                            }
                        }
                    }
                },
            },
            identity: crate::configuration::Identity {
                identity_json_file_path: {
                    let identity_path = args
                        .value_of("identity-file")
                        .unwrap_or("")
                        .parse::<PathBuf>()
                        .expect("Provided value cannot be converted to path");
                    get_final_path(&data_dir, identity_path)
                },
                expected_pow: args
                    .value_of("identity-expected-pow")
                    .unwrap_or("26.0")
                    .parse::<f64>()
                    .expect("Provided value cannot be converted to number"),
            },
            ffi: Ffi {
                protocol_runner: args
                    .value_of("protocol-runner")
                    .unwrap_or("")
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
                no_of_ffi_calls_threshold_for_gc: args
                    .value_of("ffi-calls-gc-threshold")
                    .unwrap_or("50")
                    .parse::<i32>()
                    .expect("Provided value cannot be converted to number"),
                tezos_readonly_api_pool: pool_cfg(
                    &args,
                    Ffi::TEZOS_READONLY_API_POOL_DISCRIMINATOR,
                ),
                tezos_readonly_prevalidation_api_pool: pool_cfg(
                    &args,
                    Ffi::TEZOS_READONLY_PREVALIDATION_API_POOL_DISCRIMINATOR,
                ),
                tezos_without_context_api_pool: pool_cfg(
                    &args,
                    Ffi::TEZOS_WITHOUT_CONTEXT_API_POOL_DISCRIMINATOR,
                ),
            },
            tokio_threads: args
                .value_of("tokio-threads")
                .unwrap_or("0")
                .parse::<usize>()
                .expect("Provided value cannot be converted to number"),
            tezos_network,
            enable_testchain: args
                .value_of("enable-testchain")
                .unwrap_or("false")
                .parse::<bool>()
                .expect("Provided value cannot be converted to bool"),
            validate_cfg_identity_and_stop: args.is_present("validate-cfg-identity-and-stop"),
        }
    }
}
