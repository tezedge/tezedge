// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::convert::TryInto;

use async_trait::async_trait;
use failure::{bail, format_err};
use fs_extra::dir;

use sysinfo::{ProcessExt, System, SystemExt};

use shell::stats::memory::{MemoryData, ProcessMemoryStats};

use crate::display_info::NodeInfo;
use crate::display_info::{DiskData, OcamlDiskData, TezedgeDiskData};
use crate::image::DeployMonitoringContainer;
use crate::monitors::OCAML_VOLUME_PATH;
use crate::monitors::TEZEDGE_VOLUME_PATH;

pub const TEZEDGE_PORT: u16 = 18732;
pub const OCAML_PORT: u16 = 18733;

pub struct TezedgeNode;

#[async_trait]
impl Node for TezedgeNode {
    fn collect_disk_data() -> Result<DiskData, failure::Error> {
        let disk_data = TezedgeDiskData::new(
            // dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "debugger_db"))?,
            0,
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "context"))?,
            dir::get_size(&format!(
                "{}/{}",
                TEZEDGE_VOLUME_PATH, "bootstrap_db/context"
            ))?,
            dir::get_size(&format!(
                "{}/{}",
                TEZEDGE_VOLUME_PATH, "bootstrap_db/block_storage"
            ))?,
            dir::get_size(&format!(
                "{}/{}",
                TEZEDGE_VOLUME_PATH, "bootstrap_db/context_actions"
            ))?,
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "bootstrap_db/db"))?,
        );

        Ok(disk_data.into())
    }
}

impl DeployMonitoringContainer for TezedgeNode {
    const NAME: &'static str = "deploy-monitoring-tezedge-node";
}

impl TezedgeNode {
    pub async fn collect_protocol_runners_memory_stats(
        port: u16,
    ) -> Result<Vec<ProcessMemoryStats>, failure::Error> {
        let protocol_runners: Vec<MemoryData> = match reqwest::get(&format!(
            "http://localhost:{}/stats/memory/protocol_runners",
            port
        ))
        .await
        {
            Ok(result) => result.json().await?,
            Err(e) => bail!("GET memory error: {}", e),
        };

        let memory_stats: Vec<ProcessMemoryStats> = protocol_runners
            .into_iter()
            .map(|v| v.try_into().unwrap())
            .collect();

        Ok(memory_stats)
    }
}

pub struct OcamlNode;

#[async_trait]
impl Node for OcamlNode {
    fn collect_disk_data() -> Result<DiskData, failure::Error> {
        Ok(OcamlDiskData::new(
            // dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "debugger_db"))?,
            0,
            dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "data/store"))?,
            dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "data/context"))?,
        )
        .into())
    }
}

impl DeployMonitoringContainer for OcamlNode {
    const NAME: &'static str = "deploy-monitoring-ocaml-node";
}

impl OcamlNode {
    pub fn collect_validator_memory_stats() -> Result<Vec<ProcessMemoryStats>, failure::Error> {
        let mut system = System::new_all();
        system.refresh_all();

        // collect all processes from the system
        let system_processes = system.get_processes();

        // collect all PIDs from process called tezos-node (ocaml node)
        let tezos_ocaml_processes: Vec<Option<i32>> = system_processes
            .iter()
            .filter(|(_, process)| process.name().contains("tezos-node"))
            .map(|(pid, _)| Some(*pid))
            .collect();

        // collect all processes that is the child of the main process and sum up the memory usage
        let valaidators: Vec<ProcessMemoryStats> = system_processes
            .iter()
            .filter(|(_, process)| tezos_ocaml_processes.contains(&process.parent()))
            .map(|(_, process)| {
                ProcessMemoryStats::new(
                    process.virtual_memory().try_into().unwrap_or_default(),
                    process.memory().try_into().unwrap_or_default(),
                )
            })
            .collect();

        Ok(valaidators)
    }
}

#[async_trait]
pub trait Node {
    async fn collect_head_data(port: u16) -> Result<NodeInfo, failure::Error> {
        let head_data = match reqwest::get(&format!(
            "http://localhost:{}/chains/main/blocks/head/header",
            port
        ))
        .await
        {
            Ok(result) => {
                let res_json: serde_json::Value = result.json().await?;
                NodeInfo::new(
                    res_json["level"]
                        .as_u64()
                        .ok_or_else(|| format_err!("Level is not u64"))?,
                    res_json["hash"]
                        .as_str()
                        .ok_or_else(|| format_err!("hash is not str"))?
                        .to_string(),
                    res_json["timestamp"]
                        .as_str()
                        .ok_or_else(|| format_err!("timestamp is not str"))?
                        .to_string(),
                )
            }
            Err(e) => bail!("GET header error: {}", e),
        };

        Ok(head_data)
    }

    async fn collect_memory_data(port: u16) -> Result<ProcessMemoryStats, failure::Error> {
        let tezedge_raw_memory_info: MemoryData =
            match reqwest::get(&format!("http://localhost:{}/stats/memory", port)).await {
                Ok(result) => result.json().await?,
                Err(e) => bail!("GET memory error: {}", e),
            };
        let memory_stats: ProcessMemoryStats = tezedge_raw_memory_info.try_into()?;

        Ok(memory_stats)
    }

    async fn collect_commit_hash(port: u16) -> Result<String, failure::Error> {
        let commit_hash =
            match reqwest::get(&format!("http://localhost:{}/monitor/commit_hash", port)).await {
                Ok(result) => result.text().await?,
                Err(e) => bail!("GET commit_hash error: {}", e),
            };

        Ok(commit_hash.trim_matches('"').trim_matches('\n').to_string())
    }

    fn collect_cpu_data(system: &mut System, process_name: &str) -> Result<i32, failure::Error> {
        // get node process
        Ok(system
            .get_processes()
            .iter()
            .filter(|(_, process)| process.name().contains(process_name))
            .map(|(_, process)| process.cpu_usage())
            .sum::<f32>() as i32)
    }

    fn collect_disk_data() -> Result<DiskData, failure::Error>;
}
