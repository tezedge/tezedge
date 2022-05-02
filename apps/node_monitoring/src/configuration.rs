// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use clap::{App, Arg};
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use tezos_messages::base::signature_public_key::SignaturePublicKeyHash;

use crate::node::{Node, NodeStatus, NodeType};

#[derive(Clone, Debug)]
pub struct DeployMonitoringEnvironment {
    // logging level
    pub log_level: slog::Level,

    // interval in seconds to check for new remote image
    // pub image_monitor_interval: Option<u64>,

    // interval in seconds to check for new remote image
    pub resource_monitor_interval: u64,

    pub explorer_url: Option<String>,

    // rpc server port
    pub rpc_port: u16,

    // optional proxy port to monitor
    pub proxy_port: Option<u16>,

    // Thresholds to alerts
    pub tezedge_alert_thresholds: AlertThresholds,

    // Thresholds to alerts
    pub ocaml_alert_thresholds: AlertThresholds,

    pub slack_configuration: Option<SlackConfiguration>,

    pub nodes: Vec<Node>,

    pub wait_for_nodes: bool,

    pub delegates: Option<Vec<String>>,
    pub report_each_error: bool,
    pub stats_dir: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct AlertThresholds {
    pub memory: u64,
    pub disk: u64,
    pub synchronization: i64,
    pub cpu: Option<u64>,
}

impl fmt::Display for AlertThresholds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\n\tMemory: {}MB\n\tTotal disk space: {}%\n\tCpu: {:?}%\n\tSynchronization: {}s\n",
            self.memory / 1024 / 1024,
            self.disk,
            self.cpu,
            self.synchronization,
        )
    }
}

#[derive(Clone, Debug)]
pub struct SlackConfiguration {
    // slack channel url
    pub slack_url: String,
}

fn deploy_monitoring_app() -> App<'static, 'static> {
    let app = App::new("Tezedge node monitoring app")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TezEdge and the project contributors")
        .setting(clap::AppSettings::AllArgsOverrideSelf)
        .arg(
            Arg::with_name("config-file")
                .long("config-file")
                .takes_value(true)
                .value_name("PATH")
                .help("Configuration file with start-up arguments (same format as cli arguments)")
                .validator(|v| {
                    if Path::new(&v).exists() {
                        Ok(())
                    } else {
                        Err(format!("Configuration file not found at '{}'", v))
                    }
                }),
        )
        .arg(Arg::with_name("explorer-url")
            .long("explorer-url")
            .takes_value(true)
            .multiple(false)
            .value_name("LINK")
            .help("Link to the explorer")
        )

        .arg(Arg::with_name("tezedge-nodes")
            .long("tezedge-nodes")
            .takes_value(true)
            .multiple(true)
            .value_name("TAG:PORT")
            .help("The tagged tezedge nodes to moitor running on this system. Format: TAG1:PORT1,TAG2:PORT2,TAG3:PORT3")
        )
        .arg(Arg::with_name("ocaml-nodes")
            .long("ocaml-nodes")
            .takes_value(true)
            .multiple(true)
            .value_name("TAG:PORT")
            .help("The tagged ocaml nodes to moitor running on this system. Format: TAG1:PORT1,TAG2:PORT2,TAG3:PORT3")
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
            Arg::with_name("slack-token")
                .long("slack-token")
                .takes_value(true)
                .value_name("SLACK-TOKEN")
                .help("The slack token of the bot sending messages"),
        )
        .arg(
            Arg::with_name("slack-url")
                .long("slack-url")
                .takes_value(true)
                .value_name("SLACK-URL")
                .help("The slack url of the channel to send the messages to"),
        )
        .arg(
            Arg::with_name("slack-channel-name")
                .long("slack-channel-name")
                .takes_value(true)
                .value_name("SLACK-CHANNEL-NAME")
                .help("[DEPRECATED] The slack url of the channel to send the messages to"),
        )
        .arg(
            Arg::with_name("resource-monitor-interval")
                .long("resource-monitor-interval")
                .takes_value(true)
                .value_name("RESOURCE-MONITOR-INTERVAL")
                .help("Interval in seconds to take resource utilization measurements"),
        )
        .arg(
            Arg::with_name("rpc-port")
                .long("rpc-port")
                .takes_value(true)
                .value_name("RPC-PORT")
                .help("Port number to open the monitoring rpc server on"),
        )
        .arg(
            Arg::with_name("proxy-port")
                .long("proxy-port")
                .takes_value(true)
                .value_name("RPC-PORT")
                .help("Node's proxy port"),
        )
        .arg(
            Arg::with_name("tezedge-alert-threshold-disk")
                .long("tezedge-alert-threshold-disk")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-DISK")
                .help("Thershold in bytes for critical alerts - disk"),
        )
        .arg(
            Arg::with_name("tezedge-alert-threshold-memory")
                .long("tezedge-alert-threshold-memory")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-MEMORY")
                .help("Thershold in bytes for critical alerts - memory"),
        )
        .arg(
            Arg::with_name("tezedge-alert-threshold-cpu")
                .long("tezedge-alert-threshold-cpu")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-CPU")
                .help("Thershold in % for critical alerts - cpu"),
        )
        .arg(
            Arg::with_name("tezedge-alert-threshold-synchronization")
                .long("tezedge-alert-threshold-synchronization")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-SYNCHRONIZATION")
                .help("Thershold in seconds for critical alerts - synchronization"),
        )
        .arg(
            Arg::with_name("ocaml-alert-threshold-disk")
                .long("ocaml-alert-threshold-disk")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-DISK")
                .help("Thershold in bytes for critical alerts - disk"),
        )
        .arg(
            Arg::with_name("ocaml-alert-threshold-memory")
                .long("ocaml-alert-threshold-memory")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-MEMORY")
                .help("Thershold in bytes for critical alerts - memory"),
        )
        .arg(
            Arg::with_name("ocaml-alert-threshold-cpu")
                .long("ocaml-alert-threshold-cpu")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-CPU")
                .help("Thershold in % for critical alerts - cpu"),
        )
        .arg(
            Arg::with_name("ocaml-alert-threshold-synchronization")
                .long("ocaml-alert-threshold-synchronization")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-SYNCHRONIZATION")
                .help("Thershold in seconds for critical alerts - synchronization"),
        )
        .arg(
            Arg::with_name("wait-for-nodes")
                .long("wait-for-nodes")
                .help("Waits for the defined nodes to come online."),
        )
        .arg(
            Arg::with_name("debugger-path")
                .long("debugger-path")
                .takes_value(true)
                .value_name("PATH")
                .help("Path to the debugger data")
                .validator(|v| {
                    if Path::new(&v).exists() {
                        Ok(())
                    } else {
                        Err(format!("Debugger data dir not found at '{}'", v))
                    }
                }),
        )
        .arg(
            Arg::with_name("delegate")
                .long("delegate")
                .takes_value(true)
                .value_name("PUBLIC-KEY")
                .help("Delegate to monitor")
                .validator(|v| {
                    match SignaturePublicKeyHash::from_b58_hash(&v) {
                        Ok(_) => Ok(()),
                        Err(err) => Err(format!("invalid public key: `{err}`")),
                    }
                }),
        )
        .arg(
            Arg::with_name("report-each-delegate-error")
                .long("report-each-delegate-error")
                .takes_value(false)
                .help("Report each lost block/missed endorsement")
        )
        .arg(
            Arg::with_name("delegate-errors-stats-dir")
                .long("delegate-errors-stats-dir")
                .takes_value(true)
                .value_name("DIR")
                .help("Save block and operations statistics on each missed endorsement")
                .validator(|v| {
                    if Path::new(&v).exists() {
                        Ok(())
                    } else {
                        Err(format!("Statistics data dir not found at '{}'", v))
                    }
                }),
        );
    app
}

// Validates single required arg. If missing, exit whole process
pub fn validate_required_arg(args: &clap::ArgMatches, arg_name: &str) {
    if !args.is_present(arg_name) {
        panic!("required \"{}\" arg is missing !!!", arg_name);
    }
}

fn validate_required_args(args: &clap::ArgMatches) {
    if !args.is_present("tezedge-nodes") {
        validate_required_arg(args, "ocaml-nodes");
    }

    if !args.is_present("ocaml-nodes") {
        validate_required_arg(args, "tezedge-nodes");
    }
}

fn check_slack_args(args: &clap::ArgMatches) -> Option<SlackConfiguration> {
    if args.is_present("slack-channel-name") {
        eprintln!("`slack-channel-name` option is deprecated");
    }
    if args.is_present("slack-token") {
        eprintln!("`slack-channel-name` option is deprecated");
    }

    // if any of the slack args are present, all 2 of them must be present
    if args.is_present("slack-url") {
        validate_required_arg(args, "slack-url");

        Some(SlackConfiguration {
            slack_url: args.value_of("slack-url").unwrap_or("").to_string(),
        })
    } else {
        None
    }
}

fn parse_nodes_args(
    args: &clap::ArgMatches,
    node_type: &NodeType,
    proxy_port: Option<u16>,
    debugger_path: Option<PathBuf>,
) -> Vec<Node> {
    let arg_string = match node_type {
        NodeType::Tezedge => "tezedge-nodes",
        NodeType::OCaml => "ocaml-nodes",
    };

    if let Some(vals) = args.values_of(arg_string) {
        vals.into_iter()
            .map(|val| {
                let components: Vec<&str> = val.split(':').collect();
                if components.len() == 3 {
                    let port = components[1]
                        .parse::<u16>()
                        .expect("Expected valid port number");
                    let tag = components[0].to_string();
                    let volume_path = PathBuf::from(components[2]);

                    // if we have a proxy port defined, make the proxy status online, if not keep it None indicating
                    // no proxy
                    let proxy_status = proxy_port.map(|_| NodeStatus::Online);

                    Node::new(
                        port,
                        proxy_port,
                        tag,
                        None,
                        volume_path,
                        debugger_path.clone(),
                        node_type.clone(),
                        NodeStatus::Online,
                        proxy_status,
                    )
                } else {
                    panic!("Wrong node format!!!")
                }
            })
            .collect()
    } else {
        Vec::new()
    }
}

impl DeployMonitoringEnvironment {
    pub fn from_args() -> Self {
        let app = deploy_monitoring_app();
        let args = app.clone().get_matches();

        validate_required_args(&args);
        let slack_configuration = check_slack_args(&args);

        let tezedge_alert_thresholds = AlertThresholds {
            memory: args
                .value_of("tezedge-alert-threshold-memory")
                .unwrap_or("4096")
                .parse::<u64>()
                .expect("Was expecting number of megabytes [u64]")
                * 1024
                * 1024,
            disk: args
                .value_of("tezedge-alert-threshold-disk")
                .unwrap_or("95")
                .parse::<u64>()
                .expect("Was expecting percentage [u64]"),
            synchronization: args
                .value_of("tezedge-alert-threshold-synchronization")
                .unwrap_or("300")
                .parse::<i64>()
                .expect("Was seconds [i64]"),
            cpu: args
                .value_of("tezedge-alert-threshold-cpu")
                .map(|cpu_thresh| {
                    cpu_thresh
                        .parse::<u64>()
                        .expect("Was expecting percentage [u64]")
                }),
        };

        let proxy_port = args
            .value_of("proxy-port")
            .map(|port| port.parse::<u16>().expect("Was port number [u16]"));

        let ocaml_alert_thresholds = AlertThresholds {
            memory: args
                .value_of("ocaml-alert-threshold-memory")
                .unwrap_or("6144")
                .parse::<u64>()
                .expect("Was expecting number of megabytes [u64]")
                * 1024
                * 1024,
            disk: args
                .value_of("ocaml-alert-threshold-disk")
                .unwrap_or("95")
                .parse::<u64>()
                .expect("Was expecting percentage [u64]"),
            synchronization: args
                .value_of("ocaml-alert-threshold-synchronization")
                .unwrap_or("300")
                .parse::<i64>()
                .expect("Was seconds [i64]"),
            cpu: args
                .value_of("ocaml-alert-threshold-cpu")
                .map(|cpu_thresh| {
                    cpu_thresh
                        .parse::<u64>()
                        .expect("Was expecting percentage [u64]")
                }),
        };
        let debugger_path = args.value_of("debugger-path").map(|debugger_path| {
            debugger_path
                .parse::<PathBuf>()
                .expect("The provided path is invalid")
        });

        let mut tezedge_nodes =
            parse_nodes_args(&args, &NodeType::Tezedge, proxy_port, debugger_path.clone());
        let ocaml_nodes = parse_nodes_args(&args, &NodeType::OCaml, proxy_port, debugger_path);

        // combine the nodes
        tezedge_nodes.extend(ocaml_nodes);

        DeployMonitoringEnvironment {
            log_level: args
                .value_of("log-level")
                .unwrap_or("info")
                .parse::<slog::Level>()
                .expect("Was expecting one value from slog::Level"),
            resource_monitor_interval: args
                .value_of("resource-monitor-interval")
                .unwrap_or("5")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),
            explorer_url: args
                .value_of("explorer-url")
                .map(|s| s.to_owned()),
            rpc_port: args
                .value_of("rpc-port")
                .unwrap_or("38732")
                .parse::<u16>()
                .expect("Expected u16 value of valid port number"),
            tezedge_alert_thresholds,
            ocaml_alert_thresholds,
            slack_configuration,
            nodes: tezedge_nodes,
            wait_for_nodes: args.is_present("wait-for-nodes"),
            proxy_port,
            delegates: args.values_of_lossy("delegate"),
            report_each_error: args.is_present("report-each-delegate-error"),
            stats_dir: args
                .value_of_lossy("delegate-errors-stats-dir")
                .map(|v| v.into_owned()),
        }
    }
}
