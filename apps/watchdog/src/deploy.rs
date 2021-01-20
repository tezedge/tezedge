// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use failure::bail;
use shiplift::Docker;
use slog::{info, warn, Logger};

use crate::deploy_with_compose::{
    restart_stack, shutdown_and_update, NODE_CONTAINER_NAME,
};
use crate::slack::SlackServer;
use crate::image::{changed, Node, Debugger, Explorer};

pub struct DeployMonitor {
    docker: Docker,
    slack: SlackServer,
    log: Logger,
}

impl DeployMonitor {
    pub fn new(docker: Docker, slack: SlackServer, log: Logger) -> Self {
        Self {
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
            docker,
            slack,
            ..
        } = self;

        if self.is_node_container_running().await {
            let node_updated = changed::<Node>(docker, &self.log).await?;
            let debugger_updated = changed::<Debugger>(docker, &self.log).await?;
            let explorer_updated = changed::<Explorer>(docker, &self.log).await?;
            // TODO: here restart everything,
            // but if debugger updated not need to restart explorer
            // if explorer updated, only need to restart explorer
            // if node updated, need to restart tezedge node and tezedge debugger
            // and recreate tezedge volume, but not need to restart tezos and explorer
            if node_updated || debugger_updated || explorer_updated {
                slack
                    .send_message("Updating tezedge node docker image")
                    .await?;
                info!(self.log, "Updating docker image...");
                shutdown_and_update().await;
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
            restart_stack().await;
        };

        Ok(())
    }

    async fn is_node_container_running(&self) -> bool {
        let DeployMonitor { docker, .. } = self;

        match docker.containers().get(NODE_CONTAINER_NAME).inspect().await {
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
}
