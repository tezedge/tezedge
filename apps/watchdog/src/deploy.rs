// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use failure::bail;
use shiplift::rep::ImageDetails;
use shiplift::Docker;
use slog::{error, info, warn, Logger};

use crate::deploy_with_compose::{
    restart_stack, shutdown_and_update, CONTAINER_IMAGE, NODE_CONTAINER_NAME,
};
use crate::slack::SlackServer;

pub struct DeployMonitor {
    docker: Docker,
    slack: SlackServer,
    image_tag: String,
    log: Logger,
}

impl DeployMonitor {
    pub fn new(docker: Docker, slack: SlackServer, image_tag: String, log: Logger) -> Self {
        Self {
            docker,
            slack,
            image_tag,
            log,
        }
    }

    async fn get_latest_image_hash(&self) -> Result<String, failure::Error> {
        match reqwest::get(&format!("https://hub.docker.com/v2/repositories/simplestakingcom/tezedge/tags/{}/?page_size=100", self.image_tag.replace('\"', ""))).await {
            Ok(result) => {
                let res_json: serde_json::Value = match result.json().await {
                    Ok(json) => json,
                    Err(e) => bail!("Error converting result to json: {:?}", e),
                };
                let digest = res_json["images"][0]["digest"].to_string().replace('"', "");
                Ok(format!("simplestakingcom/tezedge@{}", digest))
            }
            Err(e) => bail!("Error getting latest image: {:?}", e),
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
            image_tag,
            ..
        } = self;

        if self.is_node_container_running().await {
            match docker
                .images()
                .get(&format!("{}{}", CONTAINER_IMAGE, image_tag))
                .inspect()
                .await
            {
                Ok(image) => {
                    // We have the image, lets check stuff
                    let ImageDetails { repo_digests, .. } = image;

                    let remote_image_hash = self.get_latest_image_hash().await?;
                    let local_image_hash =
                        repo_digests.unwrap_or_else(|| vec!["".to_string()])[0].clone();

                    if local_image_hash == remote_image_hash {
                        // Do nothing, No update occured
                        info!(self.log, "No image change detected");
                    } else {
                        info!(
                            self.log,
                            "Image changed, local: {} != remote: {}",
                            local_image_hash,
                            remote_image_hash
                        );
                        slack
                            .send_message("Updating tezedge node docker image")
                            .await?;
                        info!(self.log, "Updating docker image...");
                        shutdown_and_update().await;
                    }
                }
                Err(e) => {
                    // Some other error we do not care about (Propagate the error)
                    error!(self.log, "Image inspect Error: {}", e)
                }
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
