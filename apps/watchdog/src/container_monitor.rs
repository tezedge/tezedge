// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use shiplift::Docker;
use slog::{error, info, Logger};
use tokio::task::JoinHandle;
use tokio::time::{delay_for, Duration};

use crate::deploy::DeployMonitor;
use crate::deploy_with_compose::{cleanup_docker, restart_stack, stop_with_compose};
use crate::info::InfoMonitor;
use crate::slack::SlackServer;

pub fn start_deploy_monitoring(
    slack: SlackServer,
    interval: u64,
    tag: String,
    log: Logger,
    running: Arc<AtomicBool>,
) -> JoinHandle<()> {
    let docker = Docker::new();
    let deploy_monitor = DeployMonitor::new(docker, slack, tag, log.clone());
    tokio::spawn(async move {
        while running.load(Ordering::Acquire) {
            if let Err(e) = deploy_monitor.monitor_stack().await {
                error!(log, "Deploy monitoring error: {}", e);
            }
            delay_for(Duration::from_secs(interval)).await;
        }
    })
}

pub fn start_info_monitoring(
    slack: SlackServer,
    interval: u64,
    tag: String,
    log: Logger,
    running: Arc<AtomicBool>,
) -> JoinHandle<()> {
    let info_monitor = InfoMonitor::new(slack, tag, log.clone());
    tokio::spawn(async move {
        while running.load(Ordering::Acquire) {
            if let Err(e) = info_monitor.send_monitoring_info().await {
                error!(log, "Info monitoring error: {}", e);
            }
            delay_for(Duration::from_secs(interval)).await;
        }
    })
}

pub async fn shutdown_and_cleanup(slack: SlackServer, log: Logger) -> Result<(), failure::Error> {
    slack.send_message("Manual shuttdown ").await?;
    info!(log, "Manual shutdown");

    stop_with_compose();
    cleanup_docker();

    Ok(())
}

pub async fn start_stack(slack: SlackServer, log: Logger) -> Result<(), failure::Error> {
    info!(log, "Starting tezedge stack");

    // cleanup possible dangling containers/volumes and start the stack
    restart_stack().await;
    slack.send_message("Tezedge stack started").await?;
    Ok(())
}
