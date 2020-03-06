// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::env;

use std::io::{self, BufRead};
use std::ffi::OsString;

use clap::{App, Arg};

use shell::peer_manager::Threshold;
use tezos_api::environment;
use tezos_api::environment::TezosEnvironment;

#[derive(Debug, Clone)]
pub struct P2p {
    pub listener_port: u16,
    pub bootstrap_lookup_addresses: Vec<String>,
    pub initial_peers: Vec<SocketAddr>,
    pub peer_threshold: Threshold,
}

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
    pub bootstrap_db_path: PathBuf,
    pub tezos_data_dir: PathBuf,
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
            _ => Err(format!("Unsupported variant: {}", s))
        }
    }
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub p2p: P2p,
    pub rpc: Rpc,
    pub logging: Logging,
    pub storage: Storage,

    pub record: bool,
    pub identity_json_file_path: PathBuf,
    pub tezos_network: TezosEnvironment,
    pub protocol_runner: PathBuf,
    pub no_of_ffi_calls_threshold_for_gc: i32,
    pub tokio_threads: usize
}

macro_rules! parse_validator_fn {
    ($t:ident, $err:expr) => {|v| if v.parse::<$t>().is_ok() { Ok(()) } else { Err($err.to_string()) } }
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
        .arg(Arg::with_name("bootstrap-db-path")
            .long("bootstrap-db-path")
            .takes_value(true)
            .value_name("PATH")
            .help("Path to bootstrap database directory.
                       In case it starts with ./ or ../, it is relative path to the current dir, otherwise to the --tezos-data-dir"))
        .arg(Arg::with_name("bootstrap-lookup-address")
            .long("bootstrap-lookup-address")
            .takes_value(true)
            .conflicts_with("peers")
            .help("A peers for dns lookup to get the peers to bootstrap the network from. Peers are delimited by a colon. Default: used according to --network parameter see TezosEnvironment"))
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
        .arg(Arg::with_name("network")
            .long("network")
            .takes_value(true)
            .possible_values(&["alphanet", "babylonnet", "babylon", "mainnet", "zeronet", "carthagenet", "carthage"])
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
        .arg(Arg::with_name("websocket-address")
            .long("websocket-address")
            .takes_value(true)
            .value_name("IP:PORT")
            .help("Websocket address where various node metrics and statistics are available")
            .validator(parse_validator_fn!(SocketAddr, "Value must be a valid IP:PORT")))
        .arg(Arg::with_name("monitor-port")
            .long("monitor-port")
            .takes_value(true)
            .value_name("PORT")
            .help("Port on which the Tezedge node monitoring information will be exposed")
            .validator(parse_validator_fn!(u16, "Value must be a valid port number")))
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
            .arg(Arg::with_name("tokio-threads")
                .long("tokio-threads")
                .takes_value(true)
                .value_name("NUM")
                .help("Number of threads spawned by a tokio thread pool. If value is zero, then number of threads equal to CPU cores is spawned.")
                .validator(parse_validator_fn!(usize, "Value must be a valid number")))
        .arg(Arg::with_name("record")
            .long("record")
            .takes_value(true)
            .value_name("BOOL")
            .help("Flag for turn on/off record mode"));
    app
}

// Explicitely validates all required parameters
// Flag Required=true must be handled separately as we parse args twice, 
// once to see only if confi-file arg is present and second time to parse all args
// In case some args are required=true and user provides only config-file, 
// first round of parsing would always fail then 
pub fn validate_required_args(args: &clap::ArgMatches) {
    validate_required_arg(args, "tezos-data-dir");
    validate_required_arg(args, "network");
    validate_required_arg(args, "bootstrap-db-path");
    validate_required_arg(args, "log-format");
    validate_required_arg(args, "monitor-port");
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
    validate_required_arg(args, "record");

    // "bootstrap-lookup-address", "log-file" and "peers" are not required
}

// Validates single required arg. If missing, exit whole process
pub fn validate_required_arg(args: &clap::ArgMatches, arg_name: &str) {
    if args.is_present(arg_name) == false {
        panic!("required \"{}\" arg is missing !!!", arg_name);
    }
}

// Returns final path. In case:
//      1. path is relative -> final_path = tezos_data_dir / path
//      2. path is absolute -> final_path = path
pub fn get_final_path(tezos_data_dir: &PathBuf, path: PathBuf) -> PathBuf {
    let mut final_path: PathBuf;

    // path is absolute or relative to the current dir -> start with ./ or ../
    if path.is_absolute() == true || path.starts_with(".") == true {
        final_path = path
    }
    // otherwise path is relative to the tezos-data-dir
    else {
        final_path = tezos_data_dir.to_path_buf();
        final_path.push(path);
    }

    // Tries to create final_path parent dir, if non-existing
    if let Some(parent_dir) = final_path.parent() {
        if parent_dir.exists() == false {
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
    let file = fs::File::open(&config_path).expect(format!("Unable to open config file at: {:?}", config_path).as_str());
    let reader = io::BufReader::new(file);

    let mut args: Vec<OsString> = vec![];

    let mut line_num = 0;
    for line_result in reader.lines() {
        let mut line = line_result.expect(format!("Unable to read line: {:?} from config file at: {:?}", line_num, config_path).as_str());
        line = line.trim().to_string();

        if line.is_empty() || line.starts_with("#") || line.starts_with("//") {
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
        if temp_args.is_present("config-file") == true {
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

        let data_dir: PathBuf = args.value_of("tezos-data-dir")
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
                bootstrap_lookup_addresses: args.
                    value_of("bootstrap-lookup-address")
                    .map(|addresses_str| addresses_str
                        .split(',')
                        .map(|address| address.to_string())
                        .collect()
                    ).unwrap_or_else(|| {
                    if !args.is_present("peers") {
                        match environment::TEZOS_ENV.get(&tezos_network) {
                            None => panic!("No tezos environment configured for: {:?}", tezos_network),
                            Some(cfg) => cfg.bootstrap_lookup_addresses.clone()
                        }
                    } else {
                        Vec::with_capacity(0)
                    }
                }
                ),
                initial_peers: args.value_of("peers")
                    .map(|peers_str| peers_str
                        .split(',')
                        .map(|ip_port| ip_port.parse().expect("Was expecting IP:PORT"))
                        .collect()
                    ).unwrap_or_default(),
                peer_threshold: Threshold::new(
                    args.value_of("peer-thresh-low")
                        .unwrap_or("")
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                    args.value_of("peer-thresh-high")
                        .unwrap_or("")
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                ),
            },
            rpc: crate::configuration::Rpc {
                listener_port: args
                    .value_of("rpc-port")
                    .unwrap_or("")
                    .parse::<u16>()
                    .expect("Was expecting value of rpc-port"),
                websocket_address: args.value_of("websocket-address")
                    .unwrap_or("")
                    .parse()
                    .expect("Provided value cannot be converted into valid uri"),
            },
            logging: crate::configuration::Logging {
                ocaml_log_enabled: args.value_of("ocaml-log-enabled")
                    .unwrap_or("")
                    .parse::<bool>()
                    .expect("Provided value cannot be converted to bool"),
                level: args.value_of("log-level")
                    .unwrap_or("")
                    .parse::<slog::Level>()
                    .expect("Was expecting one value from slog::Level"),
                format: args
                    .value_of("log-format")
                    .unwrap_or("")
                    .parse::<LogFormat>()
                    .expect("Was expecting 'simple' or 'json'"),
                file: {
                    let log_file_path = args.value_of("log-file")
                        .map(|v| v.parse::<PathBuf>().expect("Provided value cannot be converted to path"));

                    if let Some(path) = log_file_path {
                        Some(get_final_path(&data_dir, path))
                    } else {
                        log_file_path
                    }
                },
            },
            storage: crate::configuration::Storage {
                tezos_data_dir: data_dir.clone(),
                bootstrap_db_path: {
                    let db_path = args.value_of("bootstrap-db-path")
                        .unwrap_or("")
                        .parse::<PathBuf>()
                        .expect("Provided value cannot be converted to path");
                    get_final_path(&data_dir, db_path)
                },
            },
            identity_json_file_path: {
                let identity_path = args.value_of("identity-file")
                    .unwrap_or("")
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path");
                get_final_path(&data_dir, identity_path)
            },
            record: args.value_of("record")
                .unwrap_or("")
                .parse::<bool>()
                .expect("Provided value cannot be converted to bool"),
            protocol_runner: args
                .value_of("protocol-runner")
                .unwrap_or("")
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
            no_of_ffi_calls_threshold_for_gc: args.value_of("ffi-calls-gc-threshold")
                .unwrap_or("2000")
                .parse::<i32>()
                .expect("Provided value cannot be converted to number"),
            tokio_threads: args.value_of("tokio-threads")
                .unwrap_or("0")
                .parse::<usize>()
                .expect("Provided value cannot be converted to number"),
            tezos_network,
        }
    }
}
