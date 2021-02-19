// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use serde::Serialize;
use slog::Logger;

use crate::display_info::{
    CommitHashes, CpuData, DiskSpaceData, HeadData, ImagesInfo, MemoryData,
    TezedgeSpecificMemoryData,
};
use crate::image::{Explorer, TezedgeDebugger, WatchdogContainer};
use crate::node::{Node, OcamlNode, TezedgeNode, OCAML_PORT, TEZEDGE_PORT};
use crate::slack::SlackServer;

use crate::monitors::TEZEDGE_VOLUME_PATH;

#[derive(Serialize)]
pub struct SlackMonitorInfo {
    memory_info: MemoryData,
    head_info: HeadData,
    disk_info: DiskSpaceData,
    commit_hashes: CommitHashes,
    cpu_data: CpuData,
}

pub struct InfoMonitor {
    slack: SlackServer,
    log: Logger,
}

impl InfoMonitor {
    pub fn new(slack: SlackServer, log: Logger) -> Self {
        Self { slack, log }
    }

    /// Collects information and puts them into structs for easy slack message formating
    async fn collect_info(&self) -> Result<SlackMonitorInfo, failure::Error> {
        let InfoMonitor { log, .. } = self;

        // gets the total space on the filesystem of the specified path
        let total_disk_space = fs2::total_space(TEZEDGE_VOLUME_PATH)?;
        let free_disk_space = fs2::free_space(TEZEDGE_VOLUME_PATH)?;

        // collect memory data about light node and protocol runners
        let node_data = TezedgeNode::collect_memory_data(&log, TEZEDGE_PORT).await?;
        let protocol_runner_data =
            TezedgeNode::collect_protocol_runners_memory_stats(TEZEDGE_PORT).await?;
        let tezedge_memory_info = TezedgeSpecificMemoryData::new(node_data, protocol_runner_data);

        // collect memory data about ocaml node
        let ocaml_memory = OcamlNode::collect_memory_data(&log, OCAML_PORT).await?;

        let memory_info = MemoryData::new(ocaml_memory, tezedge_memory_info);

        // collect current head info from both nodes
        let ocaml_head_info = OcamlNode::collect_head_data(&log, OCAML_PORT).await?;
        let tezedge_head_info = TezedgeNode::collect_head_data(&log, TEZEDGE_PORT).await?;
        let head_info = HeadData::new(ocaml_head_info, tezedge_head_info);

        // collect disk usage data
        let disk_info = DiskSpaceData::new(
            total_disk_space,
            free_disk_space,
            TezedgeNode::collect_disk_data()?,
            OcamlNode::collect_disk_data()?,
        );

        // collect commit hashes of the running apps
        let ocaml_commit_hash = OcamlNode::collect_commit_hash(&log, OCAML_PORT).await?;
        let tezedge_commit_hash = TezedgeNode::collect_commit_hash(&log, TEZEDGE_PORT).await?;
        let debugger_commit_hash = TezedgeDebugger::collect_commit_hash().await?; // same as OcamlDebugger
        let explorer_commit_hash = Explorer::collect_commit_hash().await?;
        let commit_hashes = CommitHashes::new(
            ocaml_commit_hash,
            tezedge_commit_hash,
            debugger_commit_hash,
            explorer_commit_hash,
        );

        let cpu_data = CpuData::new(
            OcamlNode::collect_cpu_data("tezos-node")?,
            TezedgeNode::collect_cpu_data("light-node")?,
            TezedgeNode::collect_cpu_data("protocol-runner")?,
        );

        Ok(SlackMonitorInfo {
            memory_info,
            head_info,
            disk_info: disk_info.to_megabytes(),
            commit_hashes,
            cpu_data,
        })
    }

    pub async fn send_monitoring_info(&self) -> Result<(), failure::Error> {
        let InfoMonitor { slack, .. } = self;

        let info = self.collect_info().await?;
        let tezedge_image = TezedgeNode::image().await?;
        let debugger_image = TezedgeDebugger::image().await?;
        let explorer_image = Explorer::image().await?;
        let images_info = ImagesInfo::new(tezedge_image, debugger_image, explorer_image);

        slack
            .send_message(&format!(
                "*Stack info:*\n\n{}\n*Docker images:*```{}```\n*Latest heads:*```{}```\n*Cpu utilization:*```{}```\n*Memory stats:*```{}```\n*Disk usage:*```{}```",
                info.commit_hashes,
                images_info,
                info.head_info,
                info.cpu_data,
                info.memory_info,
                info.disk_info,
            ))
            .await?;

        Ok(())
    }
}
