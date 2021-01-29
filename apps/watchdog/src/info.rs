// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::convert::TryInto;
use std::fmt;

use failure::bail;
use serde::Serialize;
use slog::{info, Logger};
use fs_extra::dir;

use shell::stats::memory::{LinuxData, ProcessMemoryStats};

use crate::slack::SlackServer;
use crate::image::{Image, Node};

pub struct InfoMonitor {
    slack: SlackServer,
    log: Logger,
}

#[derive(Serialize)]
pub struct SlackMonitorInfo {
    memory_info: ProcessMemoryStats,
    last_head: String,
    commit_hash: String,
}

#[derive(Serialize, Debug, Default)]
pub struct DiskSpaceData {
    total_disk_space: u64,
    free_disk_space: u64,
    tezedge_debugger_disk_usage: u64,
    tezedge_node_disk_usage: u64,
    ocaml_debugger_disk_usage: u64,
    ocaml_node_disk_usage: u64,
}

impl DiskSpaceData {
    fn to_megabytes(&self) -> Self {
        Self::new(
            self.total_disk_space / 1024 / 1024,
            self.free_disk_space / 1024 / 1024,
            self.tezedge_debugger_disk_usage / 1024 / 1024,
            self.tezedge_node_disk_usage / 1024 / 1024,
            self.ocaml_debugger_disk_usage / 1024 / 1024,
            self.ocaml_node_disk_usage / 1024 / 1024,
        )
    }

    fn new(
        total_disk_space: u64,
        free_disk_space: u64,
        tezedge_debugger_disk_usage: u64,
        tezedge_node_disk_usage: u64,
        ocaml_debugger_disk_usage: u64,
        ocaml_node_disk_usage: u64,
    ) -> Self {
        Self {
            total_disk_space,
            free_disk_space,
            tezedge_debugger_disk_usage,
            tezedge_node_disk_usage,
            ocaml_debugger_disk_usage,
            ocaml_node_disk_usage,
        }
    }
}

impl fmt::Display for DiskSpaceData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, 
            "\nFile system: \n\t Total space: {}\n\t Free space: {}\n\nTezedge node:\n\t Debugger: {}\n\t Node: {}\n\nOcaml node:\n\t Debugger: {}\n\t Node: {}",
            self.total_disk_space,
            self.free_disk_space,
            self.tezedge_debugger_disk_usage,
            self.tezedge_node_disk_usage,
            self.ocaml_debugger_disk_usage,
            self.ocaml_node_disk_usage,
        )

        // writeln!(f, "File system:");
        // writeln!(f, "\t Total space: {}", self.total_disk_space);
        // writeln!(f, "\t Free space: {}", self.total_disk_space);
        // writeln!(f, "Tezedge node:");
        // writeln!(f, "\t Debugger: {}", self.tezedge_debugger_disk_usage);
        // writeln!(f, "\t Node: {}", self.tezedge_node_disk_usage);
        // writeln!(f, "Ocaml node:");
        // writeln!(f, "\t Debugger: {}", self.tezedge_debugger_disk_usage);
        // writeln!(f, "\t Node: {}", self.tezedge_node_disk_usage)
    }
}

impl InfoMonitor {
    pub fn new(slack: SlackServer, log: Logger) -> Self {
        Self {
            slack,
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

    fn collect_disk_data(&self) -> Result<DiskSpaceData, failure::Error> {
        // TODO: get this info from docker (shiplift needs to implement docker volume inspect)
        // path to the volume
        const TEZEDGE_VOLUME_PATH: &'static str = "/var/lib/docker/volumes/deploy_rust-shared-data/_data";
        const OCAML_VOLUME_PATH: &'static str = "/var/lib/docker/volumes/deploy_ocaml-shared-data/_data";

        // gets the total space on the filesystem of the specified path
        let total_disk_space = fs2::total_space(TEZEDGE_VOLUME_PATH)?;
        let free_disk_space = fs2::free_space(TEZEDGE_VOLUME_PATH)?;
        let ocaml_node_disk_usage = fs_extra::dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "data"))?;
        let tezedge_node_disk_usage_context = fs_extra::dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "context"))?;
        let tezedge_node_disk_usage_light_node = fs_extra::dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "bootstrap_db"))?;

        let tezedge_node_disk_usage = tezedge_node_disk_usage_context + tezedge_node_disk_usage_light_node;

        let ocaml_debugger_disk_usage = fs_extra::dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "debugger_db"))?;
        let tezedge_debugger_disk_usage = fs_extra::dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "debugger_db"))?;

        Ok(DiskSpaceData::new(
            total_disk_space,
            free_disk_space,
            tezedge_debugger_disk_usage,
            tezedge_node_disk_usage,
            ocaml_debugger_disk_usage,
            ocaml_node_disk_usage,
        ).to_megabytes())
    }

    pub async fn send_monitoring_info(&self) -> Result<(), failure::Error> {
        let info = self.collect_node_info().await?;
        // let disk_usage = self.collect_disk_data()?;
        let disk_usage = DiskSpaceData::default();

        let InfoMonitor {
            slack, ..
        } = self;

        slack
            .send_message(&format!(
                "Node info: \n\n Tezedge docker image: {} \n Last commit: {} \n Last head header: ```{}``` \n Memory stats[MB]: ```{}``` \n Disk usage[MB]: ```{}```",
                Node::name(),
                format!("https://github.com/simplestaking/tezedge/commit/{}", &info.commit_hash.replace('\"', "")),
                info.last_head,
                serde_json::to_string_pretty(&info.memory_info)?,
                &disk_usage,
            ))
            .await?;

        Ok(())
    }
}
