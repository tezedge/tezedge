// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use slog::{info, Drain, Level, Logger};
use tokio::signal;

mod configuration;
mod container_monitor;
mod deploy;
mod deploy_with_compose;
mod info;
mod slack;

#[tokio::main]
async fn main() {
    // parse and validate program arguments
    let env = configuration::WatchdogEnvironment::from_args();

    // create an slog logger
    let log = create_logger(env.log_level);

    info!(
        log,
        "Tezedge stack watchdog started. Image: {}", &env.image_tag
    );

    let slack_server = slack::SlackServer::new(
        env.slack_url,
        env.slack_token,
        env.slack_channel_name,
        log.clone(),
    );

    container_monitor::start_stack(slack_server.clone(), log.clone())
        .await
        .expect("Stack failed to start");

    let running = Arc::new(AtomicBool::new(true));

    let deploy_handle = container_monitor::start_deploy_monitoring(
        slack_server.clone(),
        env.monitor_interval,
        env.image_tag.clone(),
        log.clone(),
        running.clone(),
    );

    let monitor_handle = container_monitor::start_info_monitoring(
        slack_server.clone(),
        env.info_interval,
        env.image_tag,
        log.clone(),
        running.clone(),
    );

    // wait for SIGINT
    signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c event");
    info!(log, "Ctrl-c or SIGINT received!");

    // set running to false
    running.store(false, Ordering::Release);

    // drop the looping thread handles (forces exit)
    drop(monitor_handle);
    drop(deploy_handle);

    // cleanup
    info!(log, "Cleaning up containers");
    container_monitor::shutdown_and_cleanup(slack_server, log.clone())
        .await
        .expect("Cleanup failed");
    info!(log, "Shutdown complete");
}

/// Creates a slog Logger
fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .chan_size(32768)
    .overflow_strategy(slog_async::OverflowStrategy::Block)
    .build()
    .filter_level(level)
    .fuse();
    Logger::root(drain, slog::o!())
}
