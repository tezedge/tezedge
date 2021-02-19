// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::path::PathBuf;

use failure::bail;
use shiplift::Docker;
use slog::{info, warn, Logger};

use crate::deploy_with_compose::{
    restart_sandbox, restart_stack, shutdown_and_update, shutdown_and_update_sandbox,
};
use crate::image::{
    local_hash, remote_hash, Explorer, Sandbox, TezedgeDebugger, WatchdogContainer,
};
use crate::monitors::info::InfoMonitor;
use crate::node::TezedgeNode;
use crate::slack::SlackServer;

pub struct DeployMonitor {
    compose_file_path: PathBuf,
    docker: Docker,
    slack: SlackServer,
    log: Logger,
}

impl DeployMonitor {
    pub fn new(
        compose_file_path: PathBuf,
        docker: Docker,
        slack: SlackServer,
        log: Logger,
    ) -> Self {
        Self {
            compose_file_path,
            docker,
            slack,
            log,
        }
    }

    async fn collect_node_logs(&self) -> Result<serde_json::Value, failure::Error> {
        match reqwest::get("http://localhost:17732/v2/log?limit=100").await {
            Ok(result) => Ok(result.json::<serde_json::Value>().await?),
            Err(e) => bail!("Error collecting node logs from debugger: {:?}", e),
        }
    }

    pub async fn monitor_stack(&self) -> Result<(), failure::Error> {
        let DeployMonitor {
            slack,
            log,
            compose_file_path,
            ..
        } = self;

        if self.is_node_container_running().await {
            let node_updated = self.changed::<TezedgeNode>().await?;
            let debugger_updated = self.changed::<TezedgeDebugger>().await?;
            let explorer_updated = self.changed::<Explorer>().await?;
            // TODO: here restart everything,
            // but if debugger updated not need to restart explorer
            // if explorer updated, only need to restart explorer
            // if node updated, need to restart tezedge node and tezedge debugger
            // and recreate tezedge volume, but not need to restart tezos and explorer
            if node_updated || debugger_updated || explorer_updated {
                shutdown_and_update(&compose_file_path, log).await;

                // send node info after update
                let info_monitor = InfoMonitor::new(slack.clone(), self.log.clone());
                info_monitor.send_monitoring_info().await?;
            } else {
                // Do nothing, No update occurred
                info!(self.log, "No image change detected");
            }
        } else {
            warn!(self.log, "Node not running. Restarting stack");
            slack
                .send_message("Node not running. Restarting stack")
                .await?;

            self.send_log_dump().await?;
            restart_stack(&compose_file_path, log).await;
        };

        Ok(())
    }

    pub async fn monitor_sandbox_launcher(&self) -> Result<(), failure::Error> {
        let DeployMonitor {
            slack,
            log,
            compose_file_path,
            ..
        } = self;

        if self.is_sandbox_container_running().await {
            if self.changed::<Sandbox>().await? {
                shutdown_and_update_sandbox(&compose_file_path, log).await;
            } else {
                // Do nothing, No update occurred
                info!(self.log, "No image change detected");
            }
        } else {
            warn!(self.log, "Sandbox launcher not running. Restarting");
            slack
                .send_message("Sandbox launcher not running. Restarting")
                .await?;
            restart_sandbox(&compose_file_path, log).await;
        }

        Ok(())
    }

    async fn is_node_container_running(&self) -> bool {
        let DeployMonitor { docker, .. } = self;

        match docker.containers().get(TezedgeNode::NAME).inspect().await {
            Ok(container_data) => container_data.state.running,
            _ => false,
        }
    }

    async fn is_sandbox_container_running(&self) -> bool {
        let DeployMonitor { docker, .. } = self;

        match docker.containers().get(Sandbox::NAME).inspect().await {
            Ok(container_data) => container_data.state.running,
            _ => false,
        }
    }

    async fn send_log_dump(&self) -> Result<(), failure::Error> {
        let logs = serde_json::to_string(&self.collect_node_logs().await?)?;

        self.slack
            .upload_file(":warning: Logs from debugger :warning:", &logs)
            .await?;
        Ok(())
    }

    async fn changed<T: WatchdogContainer + Sync + Send>(&self) -> Result<bool, failure::Error> {
        let DeployMonitor {
            docker, slack, log, ..
        } = self;

        let image = T::image().await?;
        let local_image_hash = local_hash::<T>(&docker).await?;
        let remote_image_hash = remote_hash::<T>().await?;

        if local_image_hash != remote_image_hash {
            slog::info!(
                log,
                "Image changed, local: {} != remote: {}",
                local_image_hash,
                remote_image_hash
            );
            slack
                .send_message(&format!(
                    "{} image changing {} -> {}",
                    image, local_image_hash, remote_image_hash
                ))
                .await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
