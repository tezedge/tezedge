// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::env;

use clap::{App, Arg};

use shell::peer_manager::Threshold;
use tezos_api::environment;
use tezos_api::environment::TezosEnvironment;
use tezos_api::identity::Identity;

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
    pub tezos_network: TezosEnvironment,
    pub protocol_runner: PathBuf,

    pub identity: Identity,
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
            .arg(Arg::with_name("p2p-port")
                .long("p2p-port")
                .takes_value(true)
                .value_name("PORT")
                .help("Socket listening port for p2p for communication with tezos world")
                .validator(parse_validator_fn!(u16, "Value must be a valid port number")))
            .arg(Arg::with_name("monitor-port")
                .long("monitor-port")
                .takes_value(true)
                .value_name("PORT")
                .help("Port on which the Tezedge node monitoring information will be exposed")
                .validator(parse_validator_fn!(u16, "Value must be a valid port number")))
            .arg(Arg::with_name("rpc-port")
                .long("rpc-port")
                .takes_value(true)
                .value_name("PORT")
                .help("Rust server RPC port for communication with rust node")
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
            .arg(Arg::with_name("bootstrap-lookup-address")
                .long("bootstrap-lookup-address")
                .takes_value(true)
                .conflicts_with("peers")
                .help("A peers for dns lookup to get the peers to bootstrap the network from. Peers are delimited by a colon. Default: used according to --network parameter see TezosEnvironment"))
            .arg(Arg::with_name("bootstrap-db-path")
                .long("bootstrap-db-path")
                .takes_value(true)
                .help("Path to bootstrap database directory. Default: bootstrap_db"))
            .arg(Arg::with_name("tezos-data-dir")
                .long("tezos-data-dir")
                .takes_value(true)
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
            .arg(Arg::with_name("ocaml-log-enabled")
                .long("ocaml-log-enabled")
                .takes_value(true)
                .help("Flag for turn on/off logging in Tezos OCaml runtime. Default: false"))
            .arg(Arg::with_name("websocket-address")
                .long("websocket-address")
                .takes_value(true)
                .validator(parse_validator_fn!(SocketAddr, "Value must be a valid IP:PORT")))
            .arg(Arg::with_name("record")
                .long("record")
                .takes_value(false))
            .arg(Arg::with_name("verbose")
                .long("verbose")
                .takes_value(false)
                .multiple(true)
                .help("Turn verbose output on"))
            .arg(Arg::with_name("log-format")
                .long("log-format")
                .takes_value(true)
                .possible_values(&["json", "simple"])
                .help("Set output format of the log."))
            .arg(Arg::with_name("log-file")
                .long("log-file")
                .takes_value(true)
                .value_name("PATH")
                .help("Set path of a log file"))
            .arg(Arg::with_name("network")
                .long("network")
                .takes_value(true)
                .possible_values(&["alphanet", "babylonnet", "babylon", "mainnet", "zeronet"])
                .help("Choose the Tezos environment"))
            .arg(Arg::with_name("protocol-runner")
                .long("protocol-runner")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to a tezos protocol runner executable")
                .validator(|v| if Path::new(&v).exists() { Ok(()) } else { Err(format!("Tezos protocol runner executable not found at '{}'", v)) }))
            .arg(Arg::with_name("peer-id")
                .long("peer-id")
                .takes_value(true)
                .value_name("ID")
                .help("TODO: add desc"))
            .arg(Arg::with_name("public-key")
                .long("public-key")
                .takes_value(true)
                .value_name("KEY (HEX)")
                .help("TODO: add desc"))
            .arg(Arg::with_name("secret-key")
                .long("secret-key")
                .takes_value(true)
                .value_name("KEY (HEX)")
                .help("TODO: add desc"))
            .arg(Arg::with_name("pow-stamp")
                .long("pow-stamp")
                .takes_value(true)
                .value_name("STAMP (HEX)")
                .help("TODO: add desc"));
    app
}

// Explicitely validates all required parameters
// Flag Required=true must be handled separately as we parse args twice, 
// once to see only if confi-file arg is present and second time to parse all args
// In case some args are required=true and user provides only config-file, 
// first round of parsing would always fail then 
pub fn validate_required_args(args: &clap::ArgMatches)  {
    validate_required_arg(args, "network");
    validate_required_arg(args, "bootstrap-db-path");
    validate_required_arg(args, "log-format");
    validate_required_arg(args, "monitor-port");
    validate_required_arg(args, "ocaml-log-enabled");
    validate_required_arg(args, "p2p-port");
    
    validate_required_arg(args, "protocol-runner");
    validate_required_arg(args, "rpc-port");
    validate_required_arg(args, "tezos-data-dir");
    validate_required_arg(args, "websocket-address");

    validate_required_arg(args, "peer-id");
    validate_required_arg(args, "peer-thresh-low");
    validate_required_arg(args, "peer-thresh-high");
    validate_required_arg(args, "public-key");
    validate_required_arg(args, "secret-key");
    validate_required_arg(args, "pow-stamp");

    // "peers" and "record" are not required
}

// Validates single required arg. If missing, exit whole process
pub fn validate_required_arg(args: &clap::ArgMatches, arg_name: &str) {
    if args.is_present(arg_name) == false {
        eprintln!("required \"{}\" arg is missing !!!", arg_name);
        std::process::exit(1);
    }
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

            let mut merged_args = crate::file_config::args(config_path, false);

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
            .unwrap()
            .parse::<TezosEnvironment>()
            .expect("Was expecting one value from TezosEnvironment");

        Environment {
            p2p: crate::configuration::P2p {
                listener_port: args
                    .value_of("p2p-port")
                    .unwrap()
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
                        .unwrap()
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                    args.value_of("peer-thresh-high")
                        .unwrap()
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                ),
            },
            rpc: crate::configuration::Rpc {
                listener_port: args
                    .value_of("rpc-port")
                    .unwrap()
                    .parse::<u16>()
                    .expect("Was expecting value of rpc-port"),
                websocket_address: args.value_of("websocket-address")
                    .unwrap()
                    .parse()
                    .expect("Provided value cannot be converted into valid uri"),
            },
            logging: crate::configuration::Logging {
                ocaml_log_enabled: args.value_of("ocaml-log-enabled")
                    .unwrap()
                    .parse::<bool>()
                    .expect("Provided value cannot be converted to bool"),
                level: verbose_occurrences_to_level(args.occurrences_of("verbose")),
                format: args
                    .value_of("log-format")
                    .unwrap()
                    .parse::<LogFormat>()
                    .expect("Was expecting 'simple' or 'json'"),
                file: args.value_of("log-file")
                    .map(|v| v.parse::<PathBuf>().expect("Provided value cannot be converted to path")),
            },
            storage: crate::configuration::Storage {
                tezos_data_dir: args.value_of("tezos-data-dir")
                    .unwrap()
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
                bootstrap_db_path: args.value_of("bootstrap-db-path")
                    .unwrap()
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
            },
            identity: Identity {
                peer_id: args
                    .value_of("peer-id")
                    .unwrap()
                    .parse::<String>()
                    .expect("Expected peer-id in String format"),
                public_key: args
                    .value_of("public-key")
                    .unwrap()
                    .parse::<String>()
                    .expect("Expected public-key in String format"),
                secret_key: args
                    .value_of("secret-key")
                    .unwrap()
                    .parse::<String>()
                    .expect("Expected secret-key in String format"),   
                proof_of_work_stamp: args
                    .value_of("pow-stamp")
                    .unwrap()
                    .parse::<String>()
                    .expect("Expected pow-stamp in String format"),
            },
            record: args.is_present("record"),
            protocol_runner: args
                .value_of("protocol-runner")
                .unwrap()
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
            tezos_network,
        }
    }
}

fn verbose_occurrences_to_level(occurrences: u64) -> slog::Level {
    match occurrences {
        0 => slog::Level::Info,
        1 => slog::Level::Debug,
        _ => slog::Level::Trace,
    }
}