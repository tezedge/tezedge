// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::convert::TryInto;

use failure::bail;
use serde::Serialize;
use slog::{info, Logger};

use shell::stats::memory::{LinuxData, ProcessMemoryStats};

use crate::slack::SlackServer;

pub struct InfoMonitor {
    slack: SlackServer,
    image_tag: String,
    log: Logger,
}

#[derive(Serialize)]
pub struct SlackMonitorInfo {
    memory_info: ProcessMemoryStats,
    last_head: String,
    commit_hash: String,
}

impl InfoMonitor {
    pub fn new(slack: SlackServer, image_tag: String, log: Logger) -> Self {
        Self {
            slack,
            image_tag,
            log,
        }
    }

    async fn collect_node_info(&self) -> Result<SlackMonitorInfo, failure::Error> {
        let last_head =
            match reqwest::get("http://localhost:18732/chains/main/blocks/head/header").await {
                Ok(result) => {
                    let res_json: serde_json::Value = result.json().await?;
                    serde_json::to_string_pretty(&res_json)?
                }
                Err(e) => bail!("GET header error: {}", e),
            };
        info!(self.log, "header: {}", last_head);
        let commit_hash = match reqwest::get("http://localhost:18732/monitor/commit_hash").await {
            Ok(result) => result.text().await?,
            Err(e) => bail!("GET commit_hash error: {}", e),
        };
        info!(self.log, "commit_hash: {}", commit_hash);
        let raw_memory_info: LinuxData =
            match reqwest::get("http://localhost:18732/stats/memory").await {
                Ok(result) => result.json().await?,
                Err(e) => bail!("GET memory error: {}", e),
            };
        info!(self.log, "raw_memory_info: {:?}", raw_memory_info);

        let memory_info: ProcessMemoryStats = raw_memory_info.try_into()?;

        Ok(SlackMonitorInfo {
            last_head,
            memory_info,
            commit_hash,
        })
    }

    pub async fn send_monitoring_info(&self) -> Result<(), failure::Error> {
        let info = self.collect_node_info().await?;

        let InfoMonitor {
            slack, image_tag, ..
        } = self;

        slack
            .send_message(&format!(
                "Node info: \n\n Tezedge docker image: {} \n Last commit: {} \n Last head header: ```{}``` \n Memory stats[MB]: ```{}```",
                image_tag,
                format!("https://github.com/simplestaking/tezedge/commit/{}", &info.commit_hash.replace('\"', "")),
                info.last_head,
                serde_json::to_string_pretty(&info.memory_info)?
            ))
            .await?;

        Ok(())
    }
}
