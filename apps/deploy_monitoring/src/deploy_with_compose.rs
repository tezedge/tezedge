// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::{Command, Output};

use slog::{info, Logger};
use tokio::time::{sleep, Duration};

use crate::image::{Explorer, Sandbox, TezedgeDebugger, DeployMonitoringContainer};
use crate::node::{OcamlNode, TezedgeNode, TEZEDGE_PORT, OCAML_PORT};

pub const DEBUGGER_PORT: u16 = 17732;

// TODO: use external docker-compose for now, should we manage the images/containers directly?
pub async fn launch_stack(compose_file_path: &PathBuf, log: &Logger) {
    info!(log, "Tezedge explorer is starting");
    start_with_compose(compose_file_path, Explorer::NAME, "explorer");
    start_with_compose(compose_file_path, TezedgeDebugger::NAME, "tezedge-debugger");
    // debugger healthcheck
    while reqwest::get(&format!("http://localhost:{}/v2/log", DEBUGGER_PORT)).await.is_err() {
        sleep(Duration::from_millis(1000)).await;
    }
    info!(log, "Debugger for tezedge node is running");

    start_with_compose(compose_file_path, TezedgeNode::NAME, "tezedge-node");
    // node healthcheck
    while reqwest::get(&format!("http://localhost:{}/chains/main/blocks/head/header", TEZEDGE_PORT))
        .await
        .is_err()
    {
        sleep(Duration::from_millis(1000)).await;
    }
    info!(log, "Tezedge node is running");

    start_with_compose(compose_file_path, OcamlNode::NAME, "ocaml-node");
    // node healthcheck
    while reqwest::get(&format!("http://localhost:{}/chains/main/blocks/head/header", OCAML_PORT))
        .await
        .is_err()
    {
        sleep(Duration::from_millis(1000)).await;
    }
    info!(log, "Ocaml node is running");
}

pub async fn launch_sandbox(compose_file_path: &PathBuf, log: &Logger) {
    start_with_compose(compose_file_path, TezedgeDebugger::NAME, "tezedge-debugger");
    // debugger healthcheck
    while reqwest::get(&format!("http://localhost:{}/v2/log", DEBUGGER_PORT)).await.is_err() {
        sleep(Duration::from_millis(1000)).await;
    }
    info!(log, "Debugger for sandboxed tezedge node is running");

    // start sandbox launcher
    start_with_compose(compose_file_path, Sandbox::NAME, "tezedge-sandbox");
    // sandbox launcher healthcheck
    while reqwest::get("http://localhost:3030/list_nodes")
        .await
        .is_err()
    {
        sleep(Duration::from_millis(1000)).await;
    }
    info!(log, "Debugger for sandboxed tezedge node is running");
}

pub async fn restart_stack(compose_file_path: &PathBuf, log: &Logger, cleanup_data: bool) {
    stop_with_compose(compose_file_path);
    cleanup_docker(cleanup_data);
    launch_stack(compose_file_path, log).await;
}

pub async fn shutdown_and_update(compose_file_path: &PathBuf, log: &Logger, cleanup_data: bool) {
    stop_with_compose(compose_file_path);
    cleanup_docker_system();
    update_with_compose(compose_file_path);
    restart_stack(compose_file_path, log, cleanup_data).await;
}

pub async fn restart_sandbox(compose_file_path: &PathBuf, log: &Logger) {
    stop_with_compose(compose_file_path);
    cleanup_volumes();
    launch_sandbox(compose_file_path, log).await;
}

pub async fn shutdown_and_update_sandbox(compose_file_path: &PathBuf, log: &Logger, cleanup: bool) {
    stop_with_compose(compose_file_path);
    cleanup_docker(cleanup);
    update_with_compose(compose_file_path);
    restart_sandbox(compose_file_path, log).await;
}

pub fn cleanup_docker(cleanup_data: bool) {
    cleanup_docker_system();
    if cleanup_data {
        cleanup_volumes();
    }
}

pub fn start_with_compose(
    compose_file_path: &PathBuf,
    container_name: &str,
    service_ports_name: &str,
) -> Output {
    Command::new("docker-compose")
        .args(&[
            "-f",
            compose_file_path
                .to_str()
                .unwrap_or("apps/deploy_monitoring/docker-compose.deploy.latest.yml"),
            "run",
            "-d",
            "--name",
            container_name,
            "--service-ports",
            service_ports_name,
        ])
        .output()
        .expect("failed to execute docker-compose command")
}

pub fn stop_with_compose(compose_file_path: &PathBuf) -> Output {
    Command::new("docker-compose")
        .args(&[
            "-f",
            compose_file_path
                .to_str()
                .unwrap_or("apps/deploy_monitoring/docker-compose.deploy.latest.yml"),
            "down",
        ])
        .output()
        .expect("failed to execute docker-compose command")
}

pub fn update_with_compose(compose_file_path: &PathBuf) -> Output {
    Command::new("docker-compose")
        .args(&[
            "-f",
            compose_file_path
                .to_str()
                .unwrap_or("apps/deploy_monitoring/docker-compose.deploy.latest.yml"),
            "pull",
        ])
        .output()
        .expect("failed to execute docker-compose command")
}

pub fn cleanup_volumes() -> Output {
    Command::new("docker")
        .args(&["volume", "prune", "-f"])
        .output()
        .expect("failed to execute docker command")
}

pub fn cleanup_docker_system() -> Output {
    Command::new("docker")
        .args(&["system", "prune", "-a", "-f"])
        .output()
        .expect("failed to execute docker command")
}
