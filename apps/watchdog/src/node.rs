// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::convert::TryInto;

use async_trait::async_trait;
use failure::{bail, format_err};
use fs_extra::dir;
use slog::{info, Logger};
use merge::Merge;

use shell::stats::memory::{LinuxData, ProcessMemoryStats};

use crate::display_info::{DiskData, TezedgeDiskData, OcamlDiskData};
use crate::display_info::NodeInfo;
use crate::image::Image;
use crate::monitors::OCAML_VOLUME_PATH;
use crate::monitors::TEZEDGE_VOLUME_PATH;

pub struct TezedgeNode {}

#[async_trait]
impl Node for TezedgeNode {
    fn collect_disk_data() -> Result<DiskData, failure::Error> {
        let disk_data = TezedgeDiskData::new(
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "debugger_db"))?,
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "context"))?,
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "bootstrap_db/context"))?,
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "bootstrap_db/block_storage"))?,
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "bootstrap_db/context_actions"))?,
            dir::get_size(&format!("{}/{}", TEZEDGE_VOLUME_PATH, "bootstrap_db/db"))?,
        );

        Ok(disk_data.into())
    }
}

impl Image for TezedgeNode {
    const TAG_ENV_KEY: &'static str = "TEZEDGE_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "simplestakingcom/tezedge";
}

impl TezedgeNode {
    pub async fn collect_protocol_runners_memory_stats(port: u16) -> Result<ProcessMemoryStats, failure::Error> {
        let protocol_runners: Vec<LinuxData> = match reqwest::get(&format!(
            "http://localhost:{}/stats/memory/protocol_runners",
            port
        ))
        .await
        {
            Ok(result) => result.json().await?,
            Err(e) => bail!("GET memory error: {}", e),
        };

        let memory_stats: ProcessMemoryStats = protocol_runners.into_iter()
            .map(|v| v.try_into().unwrap())
            .fold(ProcessMemoryStats::default(), |mut acc, mem: ProcessMemoryStats| {acc.merge(mem); acc});

        Ok(memory_stats)
    }
}

pub struct OcamlNode {}

#[async_trait]
impl Node for OcamlNode {
    fn collect_disk_data() -> Result<DiskData, failure::Error> {
        Ok(OcamlDiskData::new(
            dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "debugger_db"))?,
            dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "data"))?
        ).into()
        )
    }

}

impl Image for OcamlNode {
    const TAG_ENV_KEY: &'static str = "OCAML_IMAGE_TAG";
    const IMAGE_NAME: &'static str = "tezos/tezos";
}

#[async_trait]
pub trait Node {
    async fn collect_head_data(log: &Logger, port: u16) -> Result<NodeInfo, failure::Error> {
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
                        .ok_or(format_err!("Level is not u64"))?,
                    res_json["hash"]
                        .as_str()
                        .ok_or(format_err!("hash is not str"))?
                        .to_string(),
                    res_json["timestamp"]
                        .as_str()
                        .ok_or(format_err!("timestamp is not str"))?
                        .to_string(),
                )
            }
            Err(e) => bail!("GET header error: {}", e),
        };

        info!(log, "head_data: {:?}", port);

        Ok(head_data)
    }

    async fn collect_memory_data(
        log: &Logger,
        port: u16
    ) -> Result<ProcessMemoryStats, failure::Error> {
        let tezedge_raw_memory_info: LinuxData =
            match reqwest::get(&format!("http://localhost:{}/stats/memory", port)).await {
                Ok(result) => result.json().await?,
                Err(e) => bail!("GET memory error: {}", e),
            };
        info!(
            log,
            "raw_memory_info: {:?}", tezedge_raw_memory_info
        );
        let memory_stats: ProcessMemoryStats = tezedge_raw_memory_info.try_into()?;

        Ok(memory_stats)
    }

    async fn collect_commit_hash(log: &Logger, port: u16) -> Result<String, failure::Error> {
        let commit_hash = match reqwest::get(&format!(
            "http://localhost:{}/monitor/commit_hash",
            port
        ))
        .await
        {
            Ok(result) => result.text().await?,
            Err(e) => bail!("GET commit_hash error: {}", e),
        };
        info!(log, "commit_hash: {}", commit_hash);

        Ok(commit_hash.trim_matches('"').trim_matches('\n').to_string())
    }

    fn collect_disk_data() -> Result<DiskData, failure::Error>;
}
