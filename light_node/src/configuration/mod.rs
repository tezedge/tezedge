use std::path::PathBuf;

use clap::{App, Arg};

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
    pub initial_peers: Vec<(String, u16)>,
    pub identity_json_file_path: Option<PathBuf>,
    pub log_message_contents: bool,
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
                .map(|peers_str| {
                    peers_str
                        .split(',')
                        .map(|ip_port: &str| {
                            let mut split = ip_port.splitn(2, ':');
                            (
                                split.next().expect("Was expecting IP address or hostname").to_string(),
                                split.next().map(|v| v.parse().expect("Failed to parse port number")).unwrap_or(9732),
                            )
                        })
                        .collect()
                }).unwrap_or(Vec::new()),
            identity_json_file_path: args.value_of("identity")
                .map(PathBuf::from),
            log_message_contents: args.value_of("log-message-contents")
                .unwrap()
                .parse::<bool>()
                .expect("Was expecting value of log-message-contents"),
        }
    }
}