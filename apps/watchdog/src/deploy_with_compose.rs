// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::{Command, Output};

use slog::{info, Logger};
use tokio::time::{sleep, Duration};

use crate::image::{Explorer, OcamlDebugger, Sandbox, TezedgeDebugger, WatchdogContainer};
use crate::node::{OcamlNode, TezedgeNode};

// TODO: use external docker-compose for now, should we manage the images/containers directly?
pub async fn launch_stack(compose_file_path: &PathBuf, log: &Logger) {
    start_with_compose(compose_file_path, Explorer::NAME, "explorer");
    start_with_compose(compose_file_path, TezedgeDebugger::NAME, "tezedge-debugger");
    // debugger healthcheck
    while reqwest::get("http://localhost:17732/v2/log").await.is_err() {
        sleep(Duration::from_millis(1000)).await;
    }
    info!(log, "Debugger for tezedge node is running");

    start_with_compose(compose_file_path, TezedgeNode::NAME, "tezedge-node");
    // node healthcheck
    while reqwest::get("http://localhost:18732/chains/main/blocks/head/header")
        .await
        .is_err()
    {
        sleep(Duration::from_millis(1000)).await;
    }
    info!(log, "Tezedge node is running");

    start_with_compose(compose_file_path, OcamlNode::NAME, "ocaml-node");
    start_with_compose(compose_file_path, OcamlDebugger::NAME, "ocaml-debugger");
    info!(log, "Debugger for ocaml node started");
    // node healthcheck
    while reqwest::get("http://localhost:18733/chains/main/blocks/head/header")
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
    while reqwest::get("http://localhost:17732/v2/log").await.is_err() {
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

pub async fn restart_stack(compose_file_path: &PathBuf, log: &Logger) {
    stop_with_compose(compose_file_path);
    cleanup_volumes();
    launch_stack(compose_file_path, log).await;
}

pub async fn shutdown_and_update(compose_file_path: &PathBuf, log: &Logger) {
    stop_with_compose(compose_file_path);
    cleanup_docker();
    update_with_compose(compose_file_path);
    restart_stack(compose_file_path, log).await;
}

pub async fn restart_sandbox(compose_file_path: &PathBuf, log: &Logger) {
    stop_with_compose(compose_file_path);
    cleanup_volumes();
    launch_sandbox(compose_file_path, log).await;
}

pub async fn shutdown_and_update_sandbox(compose_file_path: &PathBuf, log: &Logger) {
    stop_with_compose(compose_file_path);
    cleanup_docker();
    update_with_compose(compose_file_path);
    restart_sandbox(compose_file_path, log).await;
}

pub fn cleanup_docker() {
    cleanup_docker_system();
    cleanup_volumes();
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
                .unwrap_or("apps/watchdog/docker-compose.deploy.latest.yml"),
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
                .unwrap_or("apps/watchdog/docker-compose.deploy.latest.yml"),
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
                .unwrap_or("apps/watchdog/docker-compose.deploy.latest.yml"),
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
