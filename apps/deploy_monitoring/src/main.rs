// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use slog::{info, Drain, Level, Logger};
use tokio::signal;

mod configuration;
mod constants;
mod deploy_with_compose;
mod display_info;
mod image;
mod monitors;
mod node;
mod rpc;
mod slack;

use crate::configuration::DeployMonitoringEnvironment;
use crate::monitors::resource::ResourceUtilization;
use crate::monitors::{shutdown_and_cleanup, spawn_node_stack, spawn_sandbox};

#[tokio::main]
async fn main() {
    // parse and validate program arguments
    let env = configuration::DeployMonitoringEnvironment::from_args();

    // create an slog logger
    let log = create_logger(env.log_level);

    let DeployMonitoringEnvironment {
        slack_configuration,
        is_sandbox,
        ..
    } = env.clone();

    let slack_server = slack_configuration.map(|cfg| {
        slack::SlackServer::new(
            cfg.slack_url,
            cfg.slack_token,
            cfg.slack_channel_name,
            log.clone(),
        )
    });

    let running = Arc::new(AtomicBool::new(true));

    let thread_handles = if is_sandbox {
        info!(log, "Starting sandbox launcher monitoring");
        spawn_sandbox(env.clone(), slack_server.clone(), &log, running.clone()).await
    } else {
        spawn_node_stack(env.clone(), slack_server.clone(), &log, running.clone()).await
    };

    // wait for SIGINT
    signal::ctrl_c()
        .await
        .expect("Failed to listen for ctrl-c event");
    info!(log, "Ctrl-c or SIGINT received!");

    // set running to false
    running.store(false, Ordering::Release);

    // drop the looping thread handles (forces exit)
    for handle in thread_handles {
        drop(handle);
    }

    // cleanup
    info!(log, "Cleaning up containers");
    shutdown_and_cleanup(
        &env.compose_file_path,
        slack_server,
        &log,
        env.cleanup_volumes,
    )
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
