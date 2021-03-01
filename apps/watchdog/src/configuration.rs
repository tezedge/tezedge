// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::{Path, PathBuf};

use clap::{App, Arg};

pub struct WatchdogEnvironment {
    // logging level
    pub log_level: slog::Level,

    // slack bot token
    pub slack_token: String,

    // slack channel url
    pub slack_url: String,

    // slack channel name
    pub slack_channel_name: String,

    // interval in seconds to check for new remote image
    pub image_monitor_interval: u64,

    // interval in seconds to check for new remote image
    pub resource_monitor_interval: u64,

    // rpc server port
    pub rpc_port: u16,

    // flag for sandbox mode
    pub is_sandbox: bool,

    // Path for the compose file needed to manage the deployed containers
    pub compose_file_path: PathBuf,

    // Thresholds to alerts
    pub alert_thresholds: AlertThresholds,

    // flag for volume cleanup mode
    pub cleanup_volumes: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AlertThresholds {
    pub ram: u64,
    pub disk: u64,
    pub head: i64,
}

fn deploy_monitoring_app() -> App<'static, 'static> {
    let app = App::new("Tezedge watchdog app")
        .version("0.10.0")
        .author("SimpleStaking and the project contributors")
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
        .arg(
            Arg::with_name("compose-file-path")
                .long("compose-file-path")
                .takes_value(true)
                .value_name("PATH")
                .help("Path for the compose file needed to manage the deployed containers")
                .validator(|v| {
                    if Path::new(&v).exists() {
                        Ok(())
                    } else {
                        Err(format!("Configuration file not found at '{}'", v))
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
            Arg::with_name("image-tag")
                .long("image-tag")
                .takes_value(true)
                .value_name("IMAGE-TAG")
                .help("Image tag on docker hub to watch"),
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
                .help("The slack url of the channel to send the messages to"),
        )
        .arg(
            Arg::with_name("image-monitor-interval")
                .long("image-monitor-interval")
                .takes_value(true)
                .value_name("IMAGE-MONITOR-INTERVAL")
                .help("Interval in seconds to check for new remote images"),
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
                .help("Port number to open the watchdog rpc server on"),
        )
        .arg(
            Arg::with_name("sandbox")
                .long("sandbox")
                .help("Watch only the sandbox launcher and a debugger"),
        )
        .arg(Arg::with_name("cleanup-volumes")
            .long("cleanup-volumes")
            .help("Enable and dissable volume cleanup"))
        .arg(
            Arg::with_name("alert-threshold-disk")
                .long("alert-threshold-disk")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-DISK")
                .help("Thershold in bytes for critical alerts - disk"),
        )
        .arg(
            Arg::with_name("alert-threshold-memory")
                .long("alert-threshold-memory")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-MEMORY")
                .help("Thershold in bytes for critical alerts - memory"),
        )
        .arg(
            Arg::with_name("alert-threshold-synchronization")
                .long("alert-threshold-synchronization")
                .takes_value(true)
                .value_name("ALERT-THRESHOLD-SYNCHRONIZATION")
                .help("Thershold in seconds for critical alerts - synchronization"),
        )
        .arg(
            Arg::with_name("cleanup-volumes")
                .long("cleanup-volumes")
                .help("Enable and dissable volume cleanup"),
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
    validate_required_arg(args, "image-monitor-interval");
    // validate_required_arg(args, "resource-monitor-interval");
    // validate_required_arg(args, "info-interval");
    validate_required_arg(args, "slack-token");
    validate_required_arg(args, "slack-channel-name");
    validate_required_arg(args, "slack-url");
    validate_required_arg(args, "compose-file-path");
}

impl WatchdogEnvironment {
    pub fn from_args() -> Self {
        let app = deploy_monitoring_app();
        let args = app.clone().get_matches();

        validate_required_args(&args);

        let alert_thresholds = AlertThresholds {
            ram: args
                .value_of("alert-threshold-memory")
                .unwrap_or("10737418240")
                .parse::<u64>()
                .expect("Was expecting number of bytes [u64]"),
            disk: args
                .value_of("alert-threshold-disk")
                .unwrap_or("95")
                .parse::<u64>()
                .expect("Was expecting percentage [u64]"),
            head: args
                .value_of("alert-threshold-synchronization")
                .unwrap_or("300")
                .parse::<i64>()
                .expect("Was seconds [i64]"),
        };

        WatchdogEnvironment {
            log_level: args
                .value_of("log-level")
                .unwrap_or("info")
                .parse::<slog::Level>()
                .expect("Was expecting one value from slog::Level"),
            slack_token: args.value_of("slack-token").unwrap_or("").to_string(),
            slack_url: args.value_of("slack-url").unwrap_or("").to_string(),
            slack_channel_name: args
                .value_of("slack-channel-name")
                .unwrap_or("")
                .to_string(),
            compose_file_path: args
                .value_of("compose-file-path")
                .unwrap_or("")
                .parse::<PathBuf>()
                .expect("Expected valid path for the compose file"),
            image_monitor_interval: args
                .value_of("image-monitor-interval")
                .unwrap_or("0")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),
            resource_monitor_interval: args
                .value_of("resource-monitor-interval")
                .unwrap_or("0")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),
            rpc_port: args
                .value_of("rpc-port")
                .unwrap_or("38732")
                .parse::<u16>()
                .expect("Expected u16 value of valid port number"),
            is_sandbox: args.is_present("sandbox"),
            cleanup_volumes: args.is_present("cleanup-volumes"),
            alert_thresholds,
        }
    }
}
