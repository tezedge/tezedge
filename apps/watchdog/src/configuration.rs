// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

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

    // interval in seconds to send monitor info to slack
    pub info_interval: u64,

    // rpc server port
    pub rpc_port: u16,
}

fn deploy_monitoring_app() -> App<'static, 'static> {
    let app = App::new("Tezedge watchdog app")
        .version("0.10.0")
        .author("SimpleStaking and the project contributors")
        .setting(clap::AppSettings::AllArgsOverrideSelf)
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
            Arg::with_name("info-interval")
                .long("info-interval")
                .takes_value(true)
                .value_name("INFO-INTERVAL")
                .help("Interval in seconds to send monitor info to slack"),
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
    validate_required_arg(args, "resource-monitor-interval");
    validate_required_arg(args, "info-interval");
    validate_required_arg(args, "slack-token");
    validate_required_arg(args, "slack-channel-name");
    validate_required_arg(args, "slack-url");
}

impl WatchdogEnvironment {
    pub fn from_args() -> Self {
        let app = deploy_monitoring_app();
        let args = app.clone().get_matches();

        validate_required_args(&args);

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
            image_monitor_interval: args
                .value_of("image-monitor-interval")
                .unwrap_or("")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),
            resource_monitor_interval: args
                .value_of("resource-monitor-interval")
                .unwrap_or("")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),
            rpc_port: args
                .value_of("rpc-port")
                .unwrap_or("38732")
                .parse::<u16>()
                .expect("Expected u16 value of valid port number"),
            info_interval: args
                .value_of("info-interval")
                .unwrap_or("")
                .parse::<u64>()
                .expect("Expected u64 value of seconds"),
        }
    }
}
