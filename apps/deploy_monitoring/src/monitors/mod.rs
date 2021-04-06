// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

// use crate::monitors::resource::Alerts;
use crate::monitors::alerts::Alerts;
use shiplift::Docker;
use slog::{error, info, Logger};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};

use crate::configuration::AlertThresholds;
use crate::deploy_with_compose::{
    cleanup_docker, restart_sandbox, restart_stack, stop_with_compose,
};
use crate::monitors::deploy::DeployMonitor;
use crate::monitors::resource::{ResourceMonitor, ResourceUtilizationStorage};
use crate::slack::SlackServer;

pub mod alerts;
pub mod deploy;
pub mod resource;

// TODO: get this info from docker (shiplift needs to implement docker volume inspect)
// path to the volumes
pub const TEZEDGE_VOLUME_PATH: &str = "/var/lib/docker/volumes/deploy_monitoring_tezedge-shared-data/_data";
pub const OCAML_VOLUME_PATH: &str = "/var/lib/docker/volumes/deploy_monitoring_ocaml-shared-data/_data";

pub fn start_deploy_monitoring(
    compose_file_path: PathBuf,
    slack: SlackServer,
    interval: u64,
    log: Logger,
    running: Arc<AtomicBool>,
    cleanup_data: bool,
) -> JoinHandle<()> {
    let docker = Docker::new();
    let deploy_monitor =
        DeployMonitor::new(compose_file_path, docker, slack, log.clone(), cleanup_data);
    tokio::spawn(async move {
        while running.load(Ordering::Acquire) {
            if let Err(e) = deploy_monitor.monitor_stack().await {
                error!(log, "Deploy monitoring error: {}", e);
            }
            sleep(Duration::from_secs(interval)).await;
        }
    })
}

pub fn start_sandbox_monitoring(
    compose_file_path: PathBuf,
    slack: SlackServer,
    interval: u64,
    log: Logger,
    running: Arc<AtomicBool>,
    cleanup_data: bool,
) -> JoinHandle<()> {
    let docker = Docker::new();
    let deploy_monitor =
        DeployMonitor::new(compose_file_path, docker, slack, log.clone(), cleanup_data);
    tokio::spawn(async move {
        while running.load(Ordering::Acquire) {
            if let Err(e) = deploy_monitor.monitor_sandbox_launcher().await {
                error!(log, "Sandbox launcher monitoring error: {}", e);
            }
            sleep(Duration::from_secs(interval)).await;
        }
    })
}

pub fn start_resource_monitoring(
    interval: u64,
    log: Logger,
    running: Arc<AtomicBool>,
    ocaml_resource_utilization: ResourceUtilizationStorage,
    tezedge_resource_utilization: ResourceUtilizationStorage,
    alert_thresholds: AlertThresholds,
    slack: SlackServer,
) -> JoinHandle<()> {
    let alerts = Alerts::new(alert_thresholds);
    let mut resource_monitor = ResourceMonitor::new(
        ocaml_resource_utilization,
        tezedge_resource_utilization,
        None,
        alerts,
        log.clone(),
        slack,
    );
    tokio::spawn(async move {
        while running.load(Ordering::Acquire) {
            if let Err(e) = resource_monitor.take_measurement().await {
                error!(log, "Resource monitoring error: {}", e);
            }
            sleep(Duration::from_secs(interval)).await;
        }
    })
}

pub async fn shutdown_and_cleanup(
    compose_file_path: &PathBuf,
    slack: SlackServer,
    log: &Logger,
    cleanup_data: bool,
) -> Result<(), failure::Error> {
    slack.send_message("Manual shuttdown ").await?;
    info!(log, "Manual shutdown");

    stop_with_compose(compose_file_path);
    cleanup_docker(cleanup_data);

    Ok(())
}

pub async fn start_stack(
    compose_file_path: &PathBuf,
    slack: SlackServer,
    log: &Logger,
    cleanup_data: bool,
) -> Result<(), failure::Error> {
    info!(log, "Starting tezedge stack");

    // cleanup possible dangling containers/volumes and start the stack
    restart_stack(compose_file_path, log, cleanup_data).await;
    slack.send_message("Tezedge stack started").await?;
    Ok(())
}

pub async fn start_sandbox(
    compose_file_path: &PathBuf,
    slack: SlackServer,
    log: &Logger,
) -> Result<(), failure::Error> {
    info!(log, "Starting tezedge stack");

    // cleanup possible dangling containers/volumes and start the stack
    restart_sandbox(compose_file_path, log).await;
    slack
        .send_message("Tezedge sandbox launcher started")
        .await?;
    Ok(())
}
