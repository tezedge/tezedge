// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;
use std::path::PathBuf;
use clap::{App, Arg};

use shell::peer_manager::Threshold;
use tezos_client::environment;
use tezos_client::environment::TezosEnvironment;

use lazy_static::lazy_static;

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
    pub verbose: bool,
    pub log_format: LogFormat,
}

#[derive(Debug, Clone)]
pub struct Storage {
    pub bootstrap_db_path: PathBuf,
    pub tezos_data_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub enum LogFormat {
    Json,
    Simple
}

impl std::str::FromStr for LogFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "simple" => Ok(LogFormat::Simple),
            "json" => Ok(LogFormat::Json),
            _ => Err(format!("Unsupported format: {}", s))
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
}

impl Environment {
    pub fn from_cli_args() -> Self {
        let args = App::new("Tezos rp2p node")
            .version("0.0.1-demo")
            .author("Tezos rp2p team")
            .about("Tezos rp2p node demo")
            .arg(Arg::with_name("p2p-port")
                .short("l")
                .long("p2p-port")
                .takes_value(true)
                .default_value("9732")
                .help("Socket listening port for p2p for communication with tezos world"))
            .arg(Arg::with_name("rpc-port")
                .short("r")
                .long("rpc-port")
                .takes_value(true)
                .default_value("18732")
                .help("Rust server RPC port for communication with rust node"))
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
                .help("Path to bootstrap database directory. Default: bootstrap_db-<Tezos_network_suffix>"))
            .arg(Arg::with_name("tezos-data-dir")
                .short("d")
                .long("tezos-data-dir")
                .takes_value(true)
                .default_value("tezos_storage_db")
                .help("A directory for Tezos OCaml runtime storage (context/store)"))
            .arg(Arg::with_name("peer-thresh-low")
                .long("peer-thresh-low")
                .takes_value(true)
                .default_value("2")
                .help("Minimal number of peers to connect to"))
            .arg(Arg::with_name("peer-thresh-high")
                .long("peer-thresh-high")
                .takes_value(true)
                .default_value("15")
                .help("Maximal number of peers to connect to"))
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
                .default_value("0.0.0.0:4927"))
            .arg(Arg::with_name("record")
                .short("R")
                .long("record")
                .takes_value(false))
            .arg(Arg::with_name("verbose")
                .short("v")
                .long("verbose")
                .takes_value(false)
                .help("Turn verbose output on"))
            .arg(Arg::with_name("log-format")
                .short("f")
                .long("log-format")
                .takes_value(true)
                .default_value("simple")
                .help("Set output format of the log. Possible values: simple, json"))
            .arg(Arg::with_name("network")
                .short("n")
                .long("network")
                .takes_value(true)
                .required(true)
                .help("Choose the Tezos environment"))
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
                )
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
                verbose: args.is_present("verbose"),
                log_format: args
                    .value_of("log-format")
                    .unwrap_or_default()
                    .parse::<LogFormat>()
                    .expect("Was expecting 'simple' or 'json'"),
            },
            storage: crate::configuration::Storage {
                tezos_data_dir: args.value_of("tezos-data-dir")
                    .unwrap_or_default()
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path"),
                // default value is corrected by tezos network suffix
                bootstrap_db_path: args.value_of("bootstrap-db-path")
                    .unwrap_or(&format!("bootstrap_db-{:?}", tezos_network))
                    .parse::<PathBuf>()
                    .expect("Provided value cannot be converted to path")
            },
            identity_json_file_path: args.value_of("identity")
                .map(PathBuf::from),
            tezos_network,
            record: args.is_present("record"),
        }
    }
}