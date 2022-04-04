// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::io::Read;
use std::path::PathBuf;

use fs_extra::dir;
use getset::{CopyGetters, Getters, Setters};
use itertools::Itertools;
use merge::Merge;

use netinfo::{InoutType, NetStatistics};
use sysinfo::{ProcessExt, System, SystemExt};

use crate::display_info::NodeInfo;
use crate::display_info::{DiskData, OCamlDiskData, TezedgeDiskData};
use crate::monitors::resource::{
    DiskReadWrite, NetworkStats, ProcessCpuUsage, ResourceMonitorError, ValidatorCpuStats,
    ValidatorIOStats, ValidatorMemoryStats,
};

const MILLIS_TO_SECONDS_CONVERSION_CONSTANT: u64 = 1000;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeType {
    OCaml,
    Tezedge,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NodeStatus {
    Online,
    Offline,
}

#[derive(Clone, Debug, CopyGetters, Setters, Getters)]
pub struct Node {
    #[get_copy = "pub"]
    port: u16,

    #[get_copy = "pub"]
    proxy_port: Option<u16>,

    #[get = "pub"]
    tag: String,

    #[getset(get = "pub", set = "pub")]
    pid: Option<i32>,

    #[get = "pub"]
    volume_path: PathBuf,

    #[get = "pub"]
    debugger_path: Option<PathBuf>,

    #[get = "pub"]
    node_type: NodeType,

    #[getset(get = "pub", set = "pub")]
    node_status: NodeStatus,

    #[getset(get = "pub", set = "pub")]
    proxy_status: Option<NodeStatus>,
}

impl Node {
    pub fn new(
        port: u16,
        proxy_port: Option<u16>,
        tag: String,
        pid: Option<i32>,
        volume_path: PathBuf,
        debugger_path: Option<PathBuf>,
        node_type: NodeType,
        node_status: NodeStatus,
        proxy_status: Option<NodeStatus>,
    ) -> Self {
        Self {
            port,
            tag,
            pid,
            volume_path,
            node_type,
            proxy_port,
            node_status,
            proxy_status,
            debugger_path,
        }
    }

    pub fn get_disk_data(&self) -> DiskData {
        let volume_path = self.volume_path.as_path().display();
        if self.node_type == NodeType::Tezedge {
            // context stats DB is optional
            let context_stats =
                dir::get_size(&format!("{}/{}", volume_path, "context-stats-db")).unwrap_or(0);

            let debugger = if let Some(debugger_path) = &self.debugger_path {
                dir::get_size(&format!(
                    "{}/{}",
                    debugger_path.as_path().display(),
                    "tezedge"
                ))
                .unwrap_or(0)
            } else {
                0
            };

            let disk_data = TezedgeDiskData::new(
                debugger,
                dir::get_size(&format!("{}/{}", volume_path, "context")).unwrap_or(0),
                dir::get_size(&format!("{}/{}", volume_path, "bootstrap_db/block_storage"))
                    .unwrap_or(0),
                context_stats,
                dir::get_size(&format!("{}/{}", volume_path, "bootstrap_db/db")).unwrap_or(0),
            );

            DiskData::Tezedge(disk_data)
        } else {
            let debugger = if let Some(debugger_path) = &self.debugger_path {
                dir::get_size(&format!(
                    "{}/{}",
                    debugger_path.as_path().display(),
                    "tezedge"
                ))
                .unwrap_or(0)
            } else {
                0
            };

            DiskData::OCaml(OCamlDiskData::new(
                debugger,
                dir::get_size(&format!("{}/{}", volume_path, "data/store")).unwrap_or(0),
                dir::get_size(&format!("{}/{}", volume_path, "data/context")).unwrap_or(0),
            ))
        }
    }

    pub async fn get_head_data(&self) -> Result<NodeInfo, ResourceMonitorError> {
        let head_data: serde_json::Value = match reqwest::get(&format!(
            "http://127.0.0.1:{}/chains/main/blocks/head/header",
            self.port
        ))
        .await
        {
            Ok(result) => result.json().await.unwrap_or_default(),
            Err(e) => {
                return Err(ResourceMonitorError::NodeHeadError {
                    reason: e.to_string(),
                })
            }
        };

        let head_metadata: serde_json::Value = match reqwest::get(&format!(
            "http://127.0.0.1:{}/chains/main/blocks/head/metadata",
            self.port
        ))
        .await
        {
            Ok(result) => result.json().await.unwrap_or_default(),
            Err(e) => {
                return Err(ResourceMonitorError::NodeHeadError {
                    reason: e.to_string(),
                })
            }
        };

        Ok(NodeInfo::new(
            head_data["level"].as_u64().unwrap_or(0),
            head_data["payload_round"].as_u64().unwrap_or(0),
            head_data["hash"].as_str().unwrap_or("").to_string(),
            head_data["timestamp"].as_str().unwrap_or("").to_string(),
            head_data["proto"].as_u64().unwrap_or(0),
            head_metadata["level"]["cycle_position"]
                .as_u64()
                .unwrap_or(0),
            head_metadata["level"]["voting_period_position"]
                .as_u64()
                .unwrap_or(0),
            head_metadata["voting_period_kind"]
                .as_str()
                .unwrap_or("")
                .to_string(),
        ))
    }

    pub fn get_memory_stats(&self, system: &mut System) -> Result<u64, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            if let Some(process) = system.process(pid) {
                Ok(process.memory() * 1024)
            } else {
                Err(ResourceMonitorError::MemoryInfoError {
                    reason: format!("Process with PID {} not found", pid),
                })
            }
        } else {
            Err(ResourceMonitorError::MemoryInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub fn get_memory_stats_children(
        &self,
        system: &mut System,
        children_name: &str,
    ) -> Result<ValidatorMemoryStats, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            // collect all processes from the system
            let system_processes = system.processes();

            // collect all processes that is the child of the main process and sum up the memory usage
            let children: HashMap<String, u64> = system_processes
                .iter()
                .filter(|(_, process)| {
                    process.parent() == Some(pid) && process.name().eq(children_name)
                })
                .map(|(child_pid, child_process)| {
                    (
                        format!(
                            "{}-{}",
                            child_process.name().to_string(),
                            child_pid.to_string()
                        ),
                        (child_process.memory() * 1024),
                    )
                })
                .collect();

            let total = children.iter().map(|(_, v)| *v).sum::<u64>();

            Ok(ValidatorMemoryStats::new(total, children))
        } else {
            Err(ResourceMonitorError::MemoryInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub fn get_cpu_data(
        &self,
        system: &mut System,
    ) -> Result<ProcessCpuUsage, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            if let Some(process) = system.process(pid) {
                Ok(ProcessCpuUsage::new(
                    process.cpu_usage(),
                    process
                        .tasks
                        .iter()
                        .map(|(task_id, task)| {
                            (read_task_name_file(pid, *task_id), task.cpu_usage())
                        })
                        .collect(),
                ))
            } else {
                Err(ResourceMonitorError::CpuInfoError {
                    reason: format!("Process with PID {} not found", pid),
                })
            }
        } else {
            Err(ResourceMonitorError::CpuInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub fn get_cpu_data_children(
        &self,
        system: &mut System,
        children_name: &str,
    ) -> Result<ValidatorCpuStats, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            // collect all processes from the system
            let system_processes = system.processes();

            // collect all processes that is the child of the main process and sum up the cpu usage
            let children: HashMap<String, ProcessCpuUsage> = system_processes
                .iter()
                .filter(|(_, process)| {
                    process.parent() == Some(pid) && process.name().eq(children_name)
                })
                .map(|(child_pid, child_process)| {
                    (
                        child_process.name().to_string() + &child_pid.to_string(),
                        ProcessCpuUsage::new(
                            child_process.cpu_usage(),
                            child_process
                                .tasks
                                .iter()
                                .map(|(task_pid, task)| {
                                    (read_task_name_file(*child_pid, *task_pid), task.cpu_usage())
                                })
                                .collect(),
                        ),
                    )
                })
                .collect();

            let total = children.iter().map(|(_, v)| *v.collective()).sum::<f32>();

            Ok(ValidatorCpuStats::new(total, children))
        } else {
            Err(ResourceMonitorError::CpuInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub fn get_io_data(
        &self,
        system: &mut System,
        delta: u64,
    ) -> Result<DiskReadWrite, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            if let Some(process) = system.process(pid) {
                Ok(DiskReadWrite::new(
                    to_bytes_per_sec(process.disk_usage().read_bytes / delta),
                    to_bytes_per_sec(process.disk_usage().written_bytes / delta),
                ))
            } else {
                Err(ResourceMonitorError::IoInfoError {
                    reason: format!("Process with PID {} not found", pid),
                })
            }
        } else {
            Err(ResourceMonitorError::IoInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub fn get_io_data_children(
        &self,
        system: &mut System,
        children_name: &str,
        delta: u64,
    ) -> Result<ValidatorIOStats, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            // collect all processes from the system
            let system_processes = system.processes();

            let children: HashMap<String, DiskReadWrite> = system_processes
                .iter()
                .filter(|(_, process)| {
                    process.parent() == Some(pid) && process.name().eq(children_name)
                })
                .map(|(child_pid, child_process)| {
                    (
                        format!("{}-{}", child_process.name(), child_pid),
                        DiskReadWrite::new(
                            to_bytes_per_sec(child_process.disk_usage().read_bytes / delta),
                            to_bytes_per_sec(child_process.disk_usage().written_bytes / delta),
                        ),
                    )
                })
                .collect();

            let total = children
                .iter()
                .map(|(_, v)| v.clone())
                .fold1(|mut m1, m2| {
                    m1.merge(m2);
                    m1
                })
                .unwrap_or_default();

            Ok(ValidatorIOStats::new(total, children))
        } else {
            Err(ResourceMonitorError::IoInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub fn get_network_data(
        &self,
        statistics: &NetStatistics,
        delta: u64,
    ) -> Result<NetworkStats, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            Ok(NetworkStats::new(
                to_bytes_per_sec(
                    statistics.get_bytes_by_attr(Some(pid as u64), Some(InoutType::Outgoing), None)
                        / delta,
                ),
                to_bytes_per_sec(
                    statistics.get_bytes_by_attr(Some(pid as u64), Some(InoutType::Incoming), None)
                        / delta,
                ),
            ))
        } else {
            Err(ResourceMonitorError::NetworkInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub fn get_network_data_children(
        &self,
        statistics: &NetStatistics,
        system: &mut System,
        children_name: &str,
        delta: u64,
    ) -> Result<NetworkStats, ResourceMonitorError> {
        if let Some(pid) = self.pid {
            let system_processes = system.processes();

            let (sent, received) = system_processes
                .iter()
                .filter(|(_, process)| {
                    process.parent() == Some(pid) && process.name().eq(children_name)
                })
                .map(|(child_pid, _)| {
                    (
                        to_bytes_per_sec(
                            statistics.get_bytes_by_attr(
                                Some(*child_pid as u64),
                                Some(InoutType::Outgoing),
                                None,
                            ) / delta,
                        ),
                        to_bytes_per_sec(
                            statistics.get_bytes_by_attr(
                                Some(*child_pid as u64),
                                Some(InoutType::Incoming),
                                None,
                            ) / delta,
                        ),
                    )
                })
                .fold1(|acc, x| (acc.0 + x.0, acc.1 + x.1))
                .unwrap_or_default();
            Ok(NetworkStats::new(sent, received))
        } else {
            Err(ResourceMonitorError::NetworkInfoError {
                reason: format!("Node was not registered with PID {:?}", self.pid),
            })
        }
    }

    pub async fn is_reachable(&self) -> (bool, bool) {
        let is_node_alive = reqwest::get(&format!(
            "http://127.0.0.1:{}/chains/main/blocks/head/header",
            self.port
        ))
        .await
        .is_ok();

        if let Some(proxy_port) = self.proxy_port {
            let is_proxy_alive = reqwest::get(&format!(
                "http://127.0.0.1:{}/chains/main/blocks/head/header",
                proxy_port
            ))
            .await
            .is_ok();
            (is_node_alive, is_proxy_alive)
        } else {
            (is_node_alive, false)
        }
    }
}

fn read_task_name_file(pid: i32, task_pid: i32) -> String {
    if let Ok(f) = File::open(&format!("/proc/{}/task/{}/comm", pid, task_pid)) {
        let mut buf = BufReader::new(f);
        let mut name = String::new();

        match buf.read_to_string(&mut name) {
            Ok(_) => format!("{}-{}", &name.replace("\n", ""), task_pid.to_string()),
            // if for some reason we cannot read the thread name file, just use the task PID
            _ => task_pid.to_string(),
        }
    } else {
        // if for some reason we cannot access the thread name file, just use the task PID
        task_pid.to_string()
    }
}

fn to_bytes_per_sec(bytes_per_millis: u64) -> u64 {
    bytes_per_millis * MILLIS_TO_SECONDS_CONVERSION_CONSTANT
}
