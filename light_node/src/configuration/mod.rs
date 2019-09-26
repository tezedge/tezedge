use std::net::SocketAddr;
use std::path::PathBuf;

use clap::{App, Arg};

use shell::peer_manager::Threshold;

pub mod tezos_node;

lazy_static! {
    pub static ref ENV: Environment = Environment::from_cli_args();
}

#[derive(Debug, Clone)]
pub struct P2p {
    pub listener_port: u16,
    pub bootstrap_lookup_addresses: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Rpc {
    pub listener_port: u16,
}

#[derive(Debug, Clone)]
pub struct Environment {
    pub p2p: P2p,
    pub rpc: Rpc,
    pub initial_peers: Vec<SocketAddr>,
    pub identity_json_file_path: Option<PathBuf>,
    pub log_message_contents: bool,
    pub bootstrap_db_path: PathBuf,
    pub peer_threshold: Threshold,
    pub tezos_data_dir: PathBuf,
    pub websocket_address: SocketAddr,
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
                .default_value("boot.tzalpha.net,bootalpha.tzbeta.net")
                .help("A peers for dns lookup to get the peers to bootstrap the network from. Peers are delimited by a colon."))
            .arg(Arg::with_name("identity")
                .short("i")
                .long("identity")
                .takes_value(true)
                .help("Path to Tezos identity.json file."))
            .arg(Arg::with_name("log-message-contents")
                .short("m")
                .long("log-message-contents")
                .takes_value(true)
                .default_value("true")
                .help("Log message contents. Default: true"))
            .arg(Arg::with_name("bootstrap-db-path")
                .short("B")
                .long("bootstrap-db-path")
                .takes_value(true)
                .default_value("bootstrap_db")
                .help("Path to bootstrap database directory. Default: bootstrap_db"))
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
            .arg(Arg::with_name("tezos-data-dir")
                .short("d")
                .long("tezos-data-dir")
                .takes_value(true)
                .required(true)
                .help("A directory for Tezos OCaml runtime storage (context/store)"))
            .arg(Arg::with_name("websocket-address")
                .short("w")
                .long("websocket-address")
                .takes_value(true)
                .default_value("0.0.0.0:4927"))
            .get_matches();

        Environment {
            p2p: crate::configuration::P2p {
                listener_port: args
                    .value_of("p2p-port")
                    .unwrap_or_default()
                    .parse::<u16>()
                    .expect("Was expecting value of p2p-port"),
                bootstrap_lookup_addresses: args.
                    value_of("bootstrap-lookup-address")
                    .unwrap_or_default()
                    .parse::<String>()
                    .expect("Was expecting value of bootstrap-lookup-address")
                    .split(',')
                    .map(|peer| peer.to_string())
                    .collect(),
            },
            rpc: crate::configuration::Rpc {
                listener_port: args
                    .value_of("rpc-port")
                    .unwrap_or_default()
                    .parse::<u16>()
                    .expect("Was expecting value of rpc-port"),
            },
            initial_peers: args.value_of("peers")
                .map(|peers_str| peers_str
                    .split(',')
                    .map(|ip_port| ip_port.parse().expect("Was expecting IP:PORT"))
                    .collect()
                ).unwrap_or_default(),
            identity_json_file_path: args.value_of("identity")
                .map(PathBuf::from),
            tezos_data_dir: args.value_of("tezos-data-dir")
                .unwrap()
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
            log_message_contents: args.value_of("log-message-contents")
                .unwrap()
                .parse::<bool>()
                .expect("Was expecting value of log-message-contents"),
            bootstrap_db_path: args.value_of("bootstrap-db-path")
                .unwrap_or_default()
                .parse::<PathBuf>()
                .expect("Provided value cannot be converted to path"),
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
            websocket_address: args.value_of("websocket-address")
                .unwrap_or_default()
                .parse()
                .expect("Provided value cannot be converted into valid uri")
        }
    }
}