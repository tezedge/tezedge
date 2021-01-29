// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use serde::Serialize;
use slog::Logger;

use crate::display_info::{DiskSpaceData, HeadData, ImagesInfo, MemoryData, CommitHashes, TezedgeSpecificMemoryData};
use crate::image::{Debugger, Explorer, Image};
use crate::node::{TezedgeNode, OcamlNode, Node};
use crate::slack::SlackServer;

use crate::monitors::TEZEDGE_VOLUME_PATH;

pub const TEZEDGE_PORT: u16 = 18732;
pub const OCAML_PORT: u16 = 18733;

#[derive(Serialize)]
pub struct SlackMonitorInfo {
    memory_info: MemoryData,
    head_info: HeadData,
    disk_info: DiskSpaceData,
    commit_hashes: CommitHashes,
}

pub struct InfoMonitor {
    slack: SlackServer,
    log: Logger,
}

impl InfoMonitor {
    pub fn new(slack: SlackServer, log: Logger) -> Self {
        Self { slack, log }
    }

    async fn collect_info(&self) -> Result<SlackMonitorInfo, failure::Error> {
        let InfoMonitor { log, .. } = self;

        // gets the total space on the filesystem of the specified path
        let total_disk_space = fs2::total_space(TEZEDGE_VOLUME_PATH)?;
        let free_disk_space = fs2::free_space(TEZEDGE_VOLUME_PATH)?;

        // collect memory data about light node and protocol runners
        let tezedge_memory_info = TezedgeSpecificMemoryData::new(
            TezedgeNode::collect_memory_data(&log, TEZEDGE_PORT).await?,
            TezedgeNode::collect_protocol_runners_memory_stats(TEZEDGE_PORT).await?,
        );

        let memory_info = MemoryData::new(
            OcamlNode::collect_memory_data(&log, OCAML_PORT).await?,
            tezedge_memory_info,
        );

        let head_info = HeadData::new(
            OcamlNode::collect_head_data(&log, OCAML_PORT).await?,
            TezedgeNode::collect_head_data(&log, TEZEDGE_PORT).await?,
        );

        let disk_info = DiskSpaceData::new(
            total_disk_space,
            free_disk_space,
            TezedgeNode::collect_disk_data()?,
            OcamlNode::collect_disk_data()?,
        );

        let commit_hashes = CommitHashes::new(
            OcamlNode::collect_commit_hash(&log, OCAML_PORT).await?,
            TezedgeNode::collect_commit_hash(&log, TEZEDGE_PORT).await?,
            Debugger::collect_commit_hash().await?,
            Explorer::collect_commit_hash().await?,
        );

        Ok(SlackMonitorInfo {
            memory_info,
            head_info,
            disk_info: disk_info.to_megabytes(),
            commit_hashes,
        })
    }

    pub async fn send_monitoring_info(&self) -> Result<(), failure::Error> {
        let InfoMonitor { slack, .. } = self;

        let info = self.collect_info().await?;
        let images_info = ImagesInfo::new(TezedgeNode::name(), Debugger::name(), Explorer::name());        

        slack
            .send_message(&format!(
                "*Stack info:*\n\n{}\n*Docker images:*```{}```\n*Latest heads:*```{}```\n*Memory stats:*```{}```\n*Disk usage:*```{}```",
                info.commit_hashes,
                images_info,
                info.head_info,
                info.memory_info,
                info.disk_info,
            ))
            .await?;

        Ok(())
    }
}
