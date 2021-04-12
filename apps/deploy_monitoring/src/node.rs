// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::convert::TryInto;

use async_trait::async_trait;
use failure::{bail, format_err};
use fs_extra::dir;
// use itertools::Itertools;
// use merge::Merge;

use sysinfo::{Process, ProcessExt, System, SystemExt};

use shell::stats::memory::{MemoryData, ProcessMemoryStats, ProcessMemoryStatsMaxMerge};

use crate::constants::{DEBUGGER_VOLUME_PATH, OCAML_VOLUME_PATH, TEZEDGE_VOLUME_PATH};
use crate::display_info::NodeInfo;
use crate::display_info::{DiskData, OcamlDiskData, TezedgeDiskData};
use crate::image::DeployMonitoringContainer;

pub struct TezedgeNode;

#[async_trait]
impl Node for TezedgeNode {
    fn collect_disk_data() -> Result<DiskData, failure::Error> {
        let disk_data = TezedgeDiskData::new(
            dir::get_size(&format!("{}/{}", DEBUGGER_VOLUME_PATH, "tezedge"))?,
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

    // fn collect_protocol_runners_cpu_stats(system: &mut System, process_name: &str) -> Result<HashMap<String, i32>> {

    // }

    fn collect_cpu_data(system: &mut System, _process_name: &str) -> Result<i32, failure::Error> {
        let light_node_process_vec: Vec<i32> = system
            .get_processes()
            .iter()
            .filter(|(_, process)| process.name().contains("light-node"))
            .map(|(_, process)| process.cpu_usage() as i32)
            .collect();

        if light_node_process_vec.len() == 1 {
            Ok(light_node_process_vec[0])
        } else {
            bail!("Multiple light-node processes: TODO - TE-499")
        }
    }
}

impl DeployMonitoringContainer for TezedgeNode {
    const NAME: &'static str = "deploy-monitoring-tezedge-node";
}

fn extract_protocol_runner_endpoint(proc: &Process) -> String {
    let cmd = proc.cmd();

    let endpoint_index = cmd
        .iter()
        .position(|r| r == "--endpoint")
        .unwrap_or_default()
        + 1;

    if endpoint_index > 1 {
        // cmd[endpoint_index + 1]
        cmd.get(endpoint_index)
            .unwrap_or(&String::default())
            .clone()
    } else {
        // TODO: handle this properly?
        String::default()
    }
}

impl TezedgeNode {
    pub async fn collect_protocol_runners_memory_stats(
        _port: u16, // TODO: remove
        system: &mut System,
    ) -> Result<HashMap<String, ProcessMemoryStatsMaxMerge>, failure::Error> {
        // collect all processes from the system
        let system_processes = system.get_processes();

        // collect all PIDs from process called tezos-node (ocaml node)
        let tezedge_processes: Vec<Option<i32>> = system_processes
            .iter()
            .filter(|(_, process)| process.name().contains("light-node"))
            .map(|(pid, _)| Some(*pid))
            .collect();

        // collect all processes that is the child of the main process and sum up the memory usage
        let protocol_runners: HashMap<String, ProcessMemoryStatsMaxMerge> = system_processes
            .iter()
            .filter(|(_, process)| tezedge_processes.contains(&process.parent()))
            .map(|(_, process)| {
                (
                    extract_protocol_runner_endpoint(process),
                    ProcessMemoryStats::new(
                        process.virtual_memory().try_into().unwrap_or_default(),
                        process.memory().try_into().unwrap_or_default(),
                    )
                    .into(),
                )
            })
            .collect();

        // println!("Proto. runners: {:?}", protocol_runners);

        Ok(protocol_runners)
    }

    pub fn collect_protocol_runners_cpu_data(
        system: &mut System,
    ) -> Result<HashMap<String, i32>, failure::Error> {
        Ok(system
            .get_processes()
            .iter()
            .filter(|(_, process)| process.name().contains("protocol-runner"))
            .map(|(_, process)| {
                (
                    extract_protocol_runner_endpoint(process),
                    process.cpu_usage() as i32,
                )
            })
            .collect())
    }
}

pub struct OcamlNode;

#[async_trait]
impl Node for OcamlNode {
    fn collect_disk_data() -> Result<DiskData, failure::Error> {
        Ok(OcamlDiskData::new(
            dir::get_size(&format!("{}/{}", DEBUGGER_VOLUME_PATH, "tezos"))?,
            dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "data/store"))?,
            dir::get_size(&format!("{}/{}", OCAML_VOLUME_PATH, "data/context"))?,
        )
        .into())
    }

    fn collect_cpu_data(system: &mut System, _process_name: &str) -> Result<i32, failure::Error> {
        let (main_process_pid, _) = split_ocaml_processes(system)?;
        system
            .get_process(main_process_pid)
            .map(|proc| proc.cpu_usage() as i32)
            .ok_or(format_err!("Main ocaml process not found"))
    }
}

impl DeployMonitoringContainer for OcamlNode {
    const NAME: &'static str = "deploy-monitoring-ocaml-node";
}

fn split_ocaml_processes(system: &mut System) -> Result<(i32, Vec<i32>), failure::Error> {
    let system_processes = system.get_processes();

    // collect all PIDs from process called tezos-node (ocaml node)
    let tezos_ocaml_processes: Vec<(&i32, &Process)> = system_processes
        .iter()
        .filter(|(_, process)| process.name().contains("tezos-node"))
        .collect();

    let tezos_ocaml_processes_pids: Vec<Option<i32>> = system_processes
        .iter()
        .filter(|(_, process)| process.name().contains("tezos-node"))
        .map(|(pid, _)| Some(*pid))
        .collect();

    let validators = tezos_ocaml_processes
        .clone()
        .into_iter()
        .filter(|(_, process)| tezos_ocaml_processes_pids.contains(&process.parent()))
        .map(|(pid, _)| *pid)
        .collect();
    let main_process_vec: Vec<(&i32, &Process)> = tezos_ocaml_processes
        .into_iter()
        .filter(|(_, process)| !tezos_ocaml_processes_pids.contains(&process.parent()))
        .collect();

    // this should be 1 for now, see TE-499
    let (main_process_pid, _) = if main_process_vec.len() == 1 {
        main_process_vec[0]
    } else {
        bail!("Multiple main processe: TODO - TE-499")
    };
    Ok((*main_process_pid, validators))
}

impl OcamlNode {
    pub fn collect_validator_cpu_data(
        system: &mut System,
        _process_name: &str,
    ) -> Result<HashMap<String, i32>, failure::Error> {
        let (_, validators) = split_ocaml_processes(system)?;
        Ok(validators
            .iter()
            .map(|pid| {
                (
                    pid.to_string(),
                    system.get_process(*pid).unwrap().cpu_usage() as i32,
                )
            })
            .collect())

        // system.get_process(main_process_pid).map(|proc| proc.cpu_usage() as i32).ok_or(format_err!("Main ocaml process not found"))
    }

    pub fn collect_validator_memory_stats(
        system: &mut System,
    ) -> Result<HashMap<String, ProcessMemoryStatsMaxMerge>, failure::Error> {
        // let mut system = System::new_all();
        // system.refresh_all();

        // collect all processes from the system
        let system_processes = system.get_processes();

        // collect all PIDs from process called tezos-node (ocaml node)
        let tezos_ocaml_processes: Vec<Option<i32>> = system_processes
            .iter()
            .filter(|(_, process)| process.name().contains("tezos-node"))
            .map(|(pid, _)| Some(*pid))
            .collect();

        // collect all processes that is the child of the main process and sum up the memory usage
        let valaidators: HashMap<String, ProcessMemoryStatsMaxMerge> = system_processes
            .iter()
            .filter(|(_, process)| tezos_ocaml_processes.contains(&process.parent()))
            .map(|(pid, process)| {
                (
                    pid.to_string(),
                    ProcessMemoryStats::new(
                        process.virtual_memory().try_into().unwrap_or_default(),
                        process.memory().try_into().unwrap_or_default(),
                    )
                    .into(),
                )
            })
            .collect();
        // .fold1(|mut m1, m2| {
        //     m1.merge(m2);
        //     m1
        // })
        // .unwrap_or_default()
        // .into();

        // debug: TODO: remove
        // for (pid, process) in system_processes
        //     .iter()
        //     .filter(|(_, process)| tezos_ocaml_processes.contains(&process.parent()))
        // {
        //     println!("PID: {}", pid);
        //     println!("Name: {}", process.name());
        //     println!("CMD: {:?}", process.cmd());
        //     println!("Exe: {:?}", process.exe());
        //     println!();
        // }

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

    async fn collect_memory_data(port: u16) -> Result<ProcessMemoryStatsMaxMerge, failure::Error> {
        let tezedge_raw_memory_info: MemoryData =
            match reqwest::get(&format!("http://localhost:{}/stats/memory", port)).await {
                Ok(result) => result.json().await?,
                Err(e) => bail!("GET memory error: {}", e),
            };
        let memory_stats: ProcessMemoryStats = tezedge_raw_memory_info.try_into()?;

        Ok(memory_stats.into())
    }

    async fn collect_commit_hash(port: u16) -> Result<String, failure::Error> {
        let commit_hash =
            match reqwest::get(&format!("http://localhost:{}/monitor/commit_hash", port)).await {
                Ok(result) => result.text().await?,
                Err(e) => bail!("GET commit_hash error: {}", e),
            };

        Ok(commit_hash.trim_matches('"').trim_matches('\n').to_string())
    }

    fn collect_cpu_data(system: &mut System, process_name: &str) -> Result<i32, failure::Error>;

    fn collect_disk_data() -> Result<DiskData, failure::Error>;
}
