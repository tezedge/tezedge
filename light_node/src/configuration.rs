// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use clap::{App, Arg};

use lazy_static::lazy_static;
use shell::peer_manager::Threshold;
use tezos_api::environment;
use tezos_api::environment::TezosEnvironment;

lazy_static! {
    pub static ref ENV: Environment = Environment::from_cli_args();
}

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
    pub identity_json_file_path: Option<PathBuf>,
    pub tezos_network: TezosEnvironment,
    pub protocol_runner: PathBuf,
}

macro_rules! parse_validator_fn {
    ($t:ident, $err:expr) => {|v| if v.parse::<$t>().is_ok() { Ok(()) } else { Err($err.to_string()) } }
}

impl Environment {
    pub fn from_cli_args() -> Self {
        let args = App::new("Tezos Light Node")
            .version("0.3.1")
            .author("SimpleStaking and the project contributors")
            .about("Rust implementation of the tezos node")
            .arg(Arg::with_name("p2p-port")
                .short("l")
                .long("p2p-port")
                .takes_value(true)
                .default_value("9732")
                .help("Socket listening port for p2p for communication with tezos world")
                .validator(parse_validator_fn!(u16, "Value must be a valid port number")))
            .arg(Arg::with_name("rpc-port")
                .short("r")
                .long("rpc-port")
                .takes_value(true)
                .default_value("18732")
                .help("Rust server RPC port for communication with rust node")
                .validator(parse_validator_fn!(u16, "Value must be a valid port number")))
            .arg(Arg::with_name("peers")
                .short("p")
                .long("peers")
                .takes_value(true)
                .required(false)
                .help("A peer to bootstrap the network from. Peers are delimited by a colon."))
            .arg(Arg::with_name("bootstrap-lookup-address")
                .short("b")
                .long("bootstrap-lookup-address")
                .takes_value(true)
                .help("A peers for dns lookup to get the peers to bootstrap the network from. Peers are delimited by a colon. Default: used according to --network parameter see TezosEnvironment"))
            .arg(Arg::with_name("identity")
                .short("i")
                .long("identity")
                .takes_value(true)
                .help("Path to Tezos identity.json file."))
            .arg(Arg::with_name("bootstrap-db-path")
                .short("B")
                .long("bootstrap-db-path")
                .takes_value(true)
                .default_value("bootstrap_db")
                .help("Path to bootstrap database directory. Default: bootstrap_db"))
            .arg(Arg::with_name("tezos-data-dir")
                .short("d")
                .long("tezos-data-dir")
                .takes_value(true)
                .default_value("tezos_data_db")
                .help("A directory for Tezos OCaml runtime storage (context/store)")
                .validator(|v| {
                    let dir = Path::new(&v);
                    if dir.exists() && dir.is_dir() {
                        Ok(())
                    } else {
                        Err(format!("Required tezos data dir '{}' is not a directory or does not exist!", v))
                    }
                }))
            .arg(Arg::with_name("peer-thresh-low")
                .long("peer-thresh-low")
                .takes_value(true)
                .default_value("2")
                .help("Minimal number of peers to connect to")
                .validator(parse_validator_fn!(usize, "Value must be a valid number")))
            .arg(Arg::with_name("peer-thresh-high")
                .long("peer-thresh-high")
                .takes_value(true)
                .default_value("15")
                .help("Maximal number of peers to connect to")
                .validator(parse_validator_fn!(usize, "Value must be a valid number")))
            .arg(Arg::with_name("ocaml-log-enabled")
                .short("o")
                .long("ocaml-log-enabled")
                .takes_value(true)
                .default_value("false")
                .help("Flag for turn on/off logging in Tezos OCaml runtime. Default: false"))
            .arg(Arg::with_name("websocket-address")
                .short("w")
                .long("websocket-address")
                .takes_value(true)
                .default_value("0.0.0.0:4927")
                .validator(parse_validator_fn!(SocketAddr, "Value must be a valid IP:PORT")))
            .arg(Arg::with_name("record")
                .short("R")
                .long("record")
                .takes_value(false))
            .arg(Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .takes_value(false)
                .multiple(true)
                .help("Turn verbose output on"))
            .arg(Arg::with_name("log-format")
                .short("f")
                .long("log-format")
                .takes_value(true)
                .default_value("simple")
                .possible_values(&["json", "simple"])
                .help("Set output format of the log. Possible values: simple, json"))
            .arg(Arg::with_name("log-file")
                .short("F")
                .long("log-file")
                .takes_value(true)
                .value_name("PATH")
                .help("Set path of a log file"))
            .arg(Arg::with_name("network")
                .short("n")
                .long("network")
                .takes_value(true)
                .required(true)
                .possible_values(&["alphanet", "babylonnet", "babylon", "mainnet", "zeronet"])
                .help("Choose the Tezos environment"))
            .arg(Arg::with_name("protocol-runner")
                .short("P")
                .long("protocol-runner")
                .takes_value(true)
                .default_value("./protocol-runner")
                .value_name("PATH")
                .help("Path to a tezos protocol runner executable")
                .validator(|v| if Path::new(&v).exists() { Ok(()) } else { Err(format!("Tezos protocol runner executable not found at '{}'", v)) }))
            .get_matches();

        let tezos_network: TezosEnvironment = args
            .value_of("network")
            .unwrap()
            .parse::<TezosEnvironment>()
            .expect("Was expecting one value from TezosEnvironment");

        Environment {
            p2p: crate::configuration::P2p {
                listener_port: args
                    .value_of("p2p-port")
                    .unwrap_or_default()
                    .parse::<u16>()
                    .expect("Was expecting value of p2p-port"),
                bootstrap_lookup_addresses: args.
                    value_of("bootstrap-lookup-address")
                    .map(|addresses_str| addresses_str
                        .split(',')
                        .map(|address| address.to_string())
                        .collect()
                    ).unwrap_or_else(|| match environment::TEZOS_ENV.get(&tezos_network) {
                    None => panic!("No tezos environment configured for: {:?}", tezos_network),
                    Some(cfg) => cfg.bootstrap_lookup_addresses.clone()
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
                        .unwrap_or_default()
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                    args.value_of("peer-thresh-high")
                        .unwrap_or_default()
                        .parse::<usize>()
                        .expect("Provided value cannot be converted to number"),
                ),
            },
            rpc: crate::configuration::Rpc {
                listener_port: args
                    .value_of("rpc-port")
                    .unwrap_or_default()
                    .parse::<u16>()
                    .expect("Was expecting value of rpc-port"),
                websocket_address: args.value_of("websocket-address")
                    .unwrap_or_default()
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
                    .unwrap_or_default()
                    .parse::<LogFormat>()
                    .expect("Was expecting 'simple' or 'json'"),
                file: args.value_of("log-file")
                    .map(|v| v.parse::<PathBuf>().expect("Provided value cannot be converted to path")),
            },
            storage: crate::configuration::Storage {
                tezos_data_dir: args.value_of("tezos-data-dir")
                    .unwrap_or_default()
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
                bootstrap_db_path: args.value_of("bootstrap-db-path")
                    .unwrap_or_default()
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
            },
            identity_json_file_path: args.value_of("identity")
                .map(PathBuf::from),
            record: args.is_present("record"),
            protocol_runner: args
                .value_of("protocol-runner")
                .unwrap_or_default()
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
            tezos_network,
        }
    }
}

fn verbose_occurrences_to_level(occurrences: u64) -> slog::Level {
    match occurrences {
        1 => slog::Level::Debug,
        2 => slog::Level::Trace,
        _ => slog::Level::Info,
    }
}