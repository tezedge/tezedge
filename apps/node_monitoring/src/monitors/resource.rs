// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::convert::TryInto;
use std::hash::Hash;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use chrono::Utc;
use failure::Fail;
use getset::Getters;
use merge::Merge;
use netinfo::Netinfo;
use serde::Serialize;
use slog::{error, warn, Logger};
use sysinfo::{System, SystemExt};

use crate::display_info::{NodeInfo, OcamlDiskData, TezedgeDiskData};
use crate::monitors::alerts::Alerts;
use crate::node::{Node, NodeStatus, NodeType};
use crate::slack::SlackServer;
use crate::MEASUREMENTS_MAX_CAPACITY;

#[derive(Debug, Fail)]
pub enum ResourceMonitorError {
    /// Node is unreachable
    #[fail(
        display = "Error while getting node head information, reason: {}",
        reason
    )]
    NodeHeadError { reason: String },

    #[fail(
        display = "An error occured while monitoring node memory usage, reason: {}",
        reason
    )]
    MemoryInfoError { reason: String },

    #[fail(
        display = "An error occured while monitoring node cpu usage, reason: {}",
        reason
    )]
    CpuInfoError { reason: String },

    #[fail(
        display = "An error occured while monitoring node io usage, reason: {}",
        reason
    )]
    IoInfoError { reason: String },

    #[fail(
        display = "An error occured while monitoring node network usage, reason: {}",
        reason
    )]
    NetworkInfoError { reason: String },

    #[fail(
        display = "An error occured while monitoring node disk usage, reason: {}",
        reason
    )]
    DiskInfoError { reason: String },
}

#[derive(Clone, Debug, Getters)]
pub struct ResourceUtilizationStorage {
    #[get = "pub"]
    node: Node,

    #[get = "pub"]
    storage: Arc<RwLock<VecDeque<ResourceUtilization>>>,
}

impl ResourceUtilizationStorage {
    pub fn new(node: Node, storage: Arc<RwLock<VecDeque<ResourceUtilization>>>) -> Self {
        Self { node, storage }
    }
}

pub struct ResourceMonitor {
    resource_utilization: Vec<ResourceUtilizationStorage>,
    last_checked_head_level: HashMap<String, u64>,
    alerts: Alerts,
    log: Logger,
    slack: Option<SlackServer>,
    system: System,
    netinfo: Netinfo,
    last_refresh_time: Instant,
}

#[derive(Clone, Debug, Serialize, Getters, Default, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryStats {
    #[get = "pub(crate)"]
    node: u64,

    #[get = "pub(crate)"]
    validators: ValidatorMemoryStats,
}

#[derive(Clone, Debug, Serialize, Getters, Default, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorMemoryStats {
    #[get = "pub(crate)"]
    total: u64,

    #[get = "pub(crate)"]
    validators: HashMap<String, u64>,
}

impl ValidatorMemoryStats {
    pub fn new(total: u64, validators: HashMap<String, u64>) -> Self {
        Self { total, validators }
    }
}

#[derive(Clone, Debug, Getters, Serialize, Merge, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiskReadWrite {
    #[get = "pub(crate)"]
    #[merge(strategy = merge::num::saturating_add)]
    read_bytes_per_sec: u64,

    #[get = "pub(crate)"]
    #[merge(strategy = merge::num::saturating_add)]
    written_bytes_per_sec: u64,
}

impl DiskReadWrite {
    pub fn new(read_bytes_per_sec: u64, written_bytes_per_sec: u64) -> Self {
        Self {
            read_bytes_per_sec,
            written_bytes_per_sec,
        }
    }
}

#[derive(Clone, Debug, Getters, Serialize, Default, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IOStats {
    node: DiskReadWrite,

    #[get = "pub(crate)"]
    validators: ValidatorIOStats,
}

#[derive(Clone, Debug, Serialize, Getters, Default, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorIOStats {
    #[get = "pub(crate)"]
    total: DiskReadWrite,

    #[get = "pub(crate)"]
    validators: HashMap<String, DiskReadWrite>,
}

impl ValidatorIOStats {
    pub fn new(total: DiskReadWrite, validators: HashMap<String, DiskReadWrite>) -> Self {
        Self { total, validators }
    }
}

#[derive(Clone, Debug, Getters, Serialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkStats {
    #[get = "pub(crate)"]
    sent_bytes_per_sec: u64,

    #[get = "pub(crate)"]
    received_bytes_per_sec: u64,
}

impl NetworkStats {
    pub fn new(sent_bytes_per_sec: u64, received_bytes_per_sec: u64) -> Self {
        Self {
            sent_bytes_per_sec,
            received_bytes_per_sec,
        }
    }
}

#[derive(Clone, Debug, Serialize, Getters, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUtilization {
    #[get = "pub(crate)"]
    timestamp: i64,

    #[get = "pub(crate)"]
    memory: MemoryStats,

    #[get = "pub(crate)"]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "disk")]
    ocaml_disk: Option<OcamlDiskData>,

    #[get = "pub(crate)"]
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "disk")]
    tezedge_disk: Option<TezedgeDiskData>,

    #[get = "pub(crate)"]
    cpu: CpuStats,

    #[get = "pub(crate)"]
    io: IOStats,

    #[get = "pub(crate)"]
    network: NetworkStats,

    #[get = "pub(crate)"]
    #[serde(skip)]
    head_info: NodeInfo,

    #[get = "pub(crate)"]
    #[serde(skip)]
    total_disk_space: u64,

    #[get = "pub(crate)"]
    #[serde(skip)]
    free_disk_space: u64,
}

impl ResourceUtilization {
    pub fn merge(&self, other: Self) -> Self {
        let merged_ocaml_disk = if let (Some(ocaml_disk1), Some(ocaml_disk2)) =
            (self.ocaml_disk.as_ref(), other.ocaml_disk.as_ref())
        {
            Some(OcamlDiskData::new(
                cmp::max(ocaml_disk1.debugger(), ocaml_disk2.debugger()),
                cmp::max(ocaml_disk1.block_storage(), ocaml_disk2.block_storage()),
                cmp::max(ocaml_disk1.context_irmin(), ocaml_disk2.context_irmin()),
            ))
        } else {
            None
        };

        let merged_tezedge_disk = if let (Some(tezedge_disk1), Some(tezedge_disk2)) =
            (self.tezedge_disk.as_ref(), other.tezedge_disk.as_ref())
        {
            Some(TezedgeDiskData::new(
                cmp::max(tezedge_disk1.debugger(), tezedge_disk2.debugger()),
                cmp::max(tezedge_disk1.context_irmin(), tezedge_disk2.context_irmin()),
                cmp::max(
                    tezedge_disk1.context_merkle_rocksdb(),
                    tezedge_disk2.context_merkle_rocksdb(),
                ),
                cmp::max(tezedge_disk1.block_storage(), tezedge_disk2.block_storage()),
                cmp::max(
                    tezedge_disk1.context_actions(),
                    tezedge_disk2.context_actions(),
                ),
                cmp::max(tezedge_disk1.main_db(), tezedge_disk2.main_db()),
            ))
        } else {
            None
        };

        Self {
            timestamp: cmp::max(self.timestamp, other.timestamp),
            cpu: merge_cpu_stats(self.cpu.clone(), other.cpu),
            memory: MemoryStats {
                node: cmp::max(self.memory.node, other.memory.node),
                validators: merge_validator_memory_stats(
                    self.memory.validators.clone(),
                    other.memory.validators,
                ),
            },
            ocaml_disk: merged_ocaml_disk,
            tezedge_disk: merged_tezedge_disk,
            io: IOStats {
                node: DiskReadWrite {
                    read_bytes_per_sec: cmp::max(
                        self.io.node.read_bytes_per_sec,
                        other.io.node.read_bytes_per_sec,
                    ),
                    written_bytes_per_sec: cmp::max(
                        self.io.node.written_bytes_per_sec,
                        other.io.node.written_bytes_per_sec,
                    ),
                },
                validators: merge_validator_io_stats(
                    self.io.validators.clone(),
                    other.io.validators,
                ),
            },
            network: NetworkStats {
                received_bytes_per_sec: cmp::max(
                    self.network.received_bytes_per_sec,
                    other.network.received_bytes_per_sec,
                ),
                sent_bytes_per_sec: cmp::max(
                    self.network.sent_bytes_per_sec,
                    self.network.sent_bytes_per_sec,
                ),
            },
            // these below are not present in the FE data, do not need to merge with max strategy
            head_info: other.head_info,
            total_disk_space: other.total_disk_space,
            free_disk_space: other.free_disk_space,
        }
    }
}

#[derive(Clone, Debug, Serialize, Getters, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessCpuUsage {
    #[get = "pub(crate)"]
    collective: f32,

    #[get = "pub(crate)"]
    task_threads: HashMap<String, f32>,
}

impl ProcessCpuUsage {
    pub fn new(collective: f32, task_threads: HashMap<String, f32>) -> Self {
        Self {
            collective,
            task_threads,
        }
    }
}

#[derive(Clone, Debug, Serialize, Getters, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CpuStats {
    #[get = "pub(crate)"]
    node: ProcessCpuUsage,

    #[get = "pub(crate)"]
    validators: ValidatorCpuStats,
}

#[derive(Clone, Debug, Serialize, Getters, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ValidatorCpuStats {
    #[get = "pub(crate)"]
    total: f32,

    #[get = "pub(crate)"]
    validators: HashMap<String, ProcessCpuUsage>,
}

impl ValidatorCpuStats {
    pub fn new(total: f32, validators: HashMap<String, ProcessCpuUsage>) -> Self {
        Self { total, validators }
    }
}

impl ResourceMonitor {
    pub fn new(
        resource_utilization: Vec<ResourceUtilizationStorage>,
        last_checked_head_level: HashMap<String, u64>,
        alerts: Alerts,
        log: Logger,
        slack: Option<SlackServer>,
        netinfo: Netinfo,
    ) -> Self {
        Self {
            resource_utilization,
            last_checked_head_level,
            alerts,
            log,
            slack,
            system: System::new_all(),
            netinfo,
            last_refresh_time: Instant::now(),
        }
    }

    pub async fn take_measurement(&mut self) -> Result<(), ResourceMonitorError> {
        let ResourceMonitor {
            system,
            resource_utilization,
            log,
            last_checked_head_level,
            alerts,
            slack,
            netinfo,
            last_refresh_time,
            ..
        } = self;

        let network_statistics = netinfo.get_net_statistics();
        if netinfo.clear().is_err() {
            error!(log, "Cannot clear network statistics");
        }
        system.refresh_all();
        let current_refresh_time = Instant::now();

        let measurement_time_delta = last_refresh_time.elapsed().as_millis() as u64;

        for resource_storage in resource_utilization {
            let ResourceUtilizationStorage { node, storage } = resource_storage;

            let (node_reachable, proxy_reachable) = node.is_reachable().await;

            let node_resource_measurement = if node_reachable {
                // gets the total space on the filesystem of the specified path
                let free_disk_space = match fs2::free_space(node.volume_path()) {
                    Ok(free_space) => free_space,
                    Err(_) => {
                        return Err(ResourceMonitorError::DiskInfoError {
                            reason: format!(
                                "Cannot get free space on path {}",
                                node.volume_path().display()
                            ),
                        })
                    }
                };

                let total_disk_space = match fs2::total_space(node.volume_path()) {
                    Ok(total_space) => total_space,
                    Err(_) => {
                        return Err(ResourceMonitorError::DiskInfoError {
                            reason: format!(
                                "Cannot get total space on path {}",
                                node.volume_path().display()
                            ),
                        })
                    }
                };

                let current_head_info = node.get_head_data().await?;
                let node_memory = node.get_memory_stats(system)?;
                let node_disk = node.get_disk_data();
                let node_cpu = node.get_cpu_data(system)?;
                let node_io = node.get_io_data(system, measurement_time_delta)?;

                if node.node_type() == &NodeType::Tezedge {
                    let validators_memory =
                        node.get_memory_stats_children(system, "protocol-runner")?;
                    let validators_cpu = node.get_cpu_data_children(system, "protocol-runner")?;

                    let validators_io = node.get_io_data_children(
                        system,
                        "protocol-runner",
                        measurement_time_delta,
                    )?;

                    let network_stats = if let Ok(ref network_statistics) = network_statistics {
                        let node_network =
                            node.get_network_data(&network_statistics, measurement_time_delta)?;
                        let children_network = node.get_network_data_children(
                            &network_statistics,
                            system,
                            "protocol-runner",
                            measurement_time_delta,
                        )?;
                        NetworkStats {
                            sent_bytes_per_sec: node_network.sent_bytes_per_sec
                                + children_network.sent_bytes_per_sec,
                            received_bytes_per_sec: node_network.received_bytes_per_sec
                                + children_network.received_bytes_per_sec,
                        }
                    } else {
                        NetworkStats::default()
                    };

                    ResourceUtilization {
                        timestamp: chrono::Local::now().timestamp(),
                        memory: MemoryStats {
                            node: node_memory,
                            validators: validators_memory,
                        },
                        tezedge_disk: Some(node_disk.try_into()?),
                        ocaml_disk: None,
                        cpu: CpuStats {
                            node: node_cpu,
                            validators: validators_cpu,
                        },
                        io: IOStats {
                            node: node_io,
                            validators: validators_io,
                        },
                        head_info: current_head_info,
                        network: network_stats,
                        total_disk_space,
                        free_disk_space,
                    }
                } else {
                    let validators_memory = node.get_memory_stats_children(system, "tezos-node")?;
                    let validators_cpu = node.get_cpu_data_children(system, "tezos-node")?;

                    let validators_io =
                        node.get_io_data_children(system, "tezos-node", measurement_time_delta)?;

                    let network_stats = match network_statistics {
                        Ok(ref network_statistics) => {
                            let node_network =
                                node.get_network_data(&network_statistics, measurement_time_delta)?;
                            let children_network = node.get_network_data_children(
                                &network_statistics,
                                system,
                                "tezos-node",
                                measurement_time_delta,
                            )?;
                            NetworkStats {
                                sent_bytes_per_sec: node_network.sent_bytes_per_sec
                                    + children_network.sent_bytes_per_sec,
                                received_bytes_per_sec: node_network.received_bytes_per_sec
                                    + children_network.received_bytes_per_sec,
                            }
                        }
                        Err(ref e) => {
                            warn!(log, "Error getting network stats: {}", e);
                            NetworkStats::default()
                        }
                    };

                    ResourceUtilization {
                        timestamp: chrono::Local::now().timestamp(),
                        memory: MemoryStats {
                            node: node_memory,
                            validators: validators_memory,
                        },
                        ocaml_disk: Some(node_disk.try_into()?),
                        tezedge_disk: None,
                        cpu: CpuStats {
                            node: node_cpu,
                            validators: validators_cpu,
                        },
                        io: IOStats {
                            node: node_io,
                            validators: validators_io,
                        },
                        head_info: current_head_info,
                        network: network_stats,
                        total_disk_space,
                        free_disk_space,
                    }
                }
            } else {
                if !node_reachable && node.node_status() == &NodeStatus::Online {
                    println!("[{}] Node is down", node.tag());
                    if let Some(slack) = slack {
                        slack
                            .send_message(&format!("[{}] Node is down", node.tag()))
                            .await;
                    }
                    node.set_node_status(NodeStatus::Offline);
                }
                ResourceUtilization::default()
            };

            if let Some(proxy_status) = node.proxy_status() {
                if !proxy_reachable {
                    // if the proxy is not reachable and the last status was Online report trough slack and change the
                    // status to offline
                    if proxy_status == &NodeStatus::Online {
                        println!("[{}] Proxy is down", node.tag());
                        if let Some(slack) = slack {
                            slack
                                .send_message(&format!("[{}] Proxy is down", node.tag()))
                                .await;
                        }
                        node.set_proxy_status(Some(NodeStatus::Offline));
                    }
                } else {
                    // if the proxy is reachable and the last status was Offline report trough slack and change the
                    // status to online
                    if proxy_status == &NodeStatus::Offline {
                        if let Some(slack) = slack {
                            slack
                                .send_message(&format!("[{}] Proxy is back online", node.tag()))
                                .await;
                        }
                        node.set_proxy_status(Some(NodeStatus::Online));
                    }
                }
            }

            if node_reachable {
                handle_alerts(
                    node.node_type(),
                    node.tag(),
                    node_resource_measurement.clone(),
                    last_checked_head_level,
                    slack.clone(),
                    alerts,
                    log,
                )
                .await;

                match &mut storage.write() {
                    Ok(resources_locked) => {
                        if resources_locked.len() == MEASUREMENTS_MAX_CAPACITY {
                            resources_locked.pop_back();
                        }
                        resources_locked.push_front(node_resource_measurement.clone());
                    }
                    Err(e) => error!(log, "Resource lock poisoned, reason => {}", e),
                }
            }
        }
        *last_refresh_time = current_refresh_time;
        Ok(())
    }
}

async fn handle_alerts(
    node_type: &NodeType,
    node_tag: &str,
    last_measurement: ResourceUtilization,
    last_checked_head_level: &mut HashMap<String, u64>,
    slack: Option<SlackServer>,
    alerts: &mut Alerts,
    log: &Logger,
) {
    let thresholds = match *node_type {
        NodeType::Tezedge => *alerts.tezedge_thresholds(),
        NodeType::Ocaml => *alerts.ocaml_thresholds(),
    };

    // current time timestamp
    let current_time = Utc::now().timestamp();

    let last_head = last_checked_head_level.get(node_tag).copied();
    let current_head_info = last_measurement.head_info.clone();

    alerts
        .check_disk_alert(
            node_tag,
            &thresholds,
            slack.as_ref(),
            current_time,
            current_head_info.clone(),
            last_measurement.clone(),
        )
        .await;
    alerts
        .check_memory_alert(
            node_tag,
            &thresholds,
            slack.as_ref(),
            current_time,
            last_measurement.clone(),
        )
        .await;
    alerts
        .check_node_stuck_alert(
            node_tag,
            &thresholds,
            last_head,
            current_time,
            slack.as_ref(),
            log,
            current_head_info.clone(),
        )
        .await;

    if let Some(cpu_threshold) = thresholds.cpu {
        alerts
            .check_cpu_alert(
                node_tag,
                cpu_threshold,
                slack.as_ref(),
                current_time,
                last_measurement.clone(),
                current_head_info.clone(),
            )
            .await;
    }

    last_checked_head_level.insert(node_tag.to_string(), *current_head_info.level());
}

/// Merges 2 maps by picking the max value for each key, if the key is not present in both, its is added to the map
fn max_merge_maps<K: Hash + Eq + Clone, V: Ord + Default + Clone>(
    first_map: HashMap<K, V>,
    second_map: HashMap<K, V>,
) -> HashMap<K, V> {
    let mut new_map = HashMap::new();
    for (key, value) in first_map.iter() {
        new_map.insert(
            key.clone(),
            cmp::max(
                value.clone(),
                second_map.get(&key).unwrap_or(&V::default()).clone(),
            ),
        );
    }

    for (key, value) in second_map.iter() {
        new_map.insert(
            key.clone(),
            cmp::max(
                value.clone(),
                first_map.get(&key).unwrap_or(&V::default()).clone(),
            ),
        );
    }

    new_map
}

/// Merges 2 float maps by picking the max value for each key, if the key is not present in both, its is added to the map
// we need a separate function for f32 as they do not implement the Ord trait
fn max_merge_float_maps<K: Hash + Eq + Clone>(
    first_map: HashMap<K, f32>,
    second_map: HashMap<K, f32>,
) -> HashMap<K, f32> {
    // ...
    let mut new_map = HashMap::new();
    for (key, value) in first_map.iter() {
        new_map.insert(
            key.clone(),
            f32::max(*value, *second_map.get(&key).unwrap_or(&0.0)),
        );
    }

    for (key, value) in second_map.iter() {
        new_map.insert(
            key.clone(),
            f32::max(*value, *first_map.get(&key).unwrap_or(&0.0)),
        );
    }
    new_map
    // ...
}

fn merge_cpu_stats(first: CpuStats, second: CpuStats) -> CpuStats {
    let node = ProcessCpuUsage {
        collective: f32::max(first.node.collective, second.node.collective),
        task_threads: max_merge_float_maps(first.node.task_threads, second.node.task_threads),
    };

    let mut validators_merged = HashMap::new();

    for (key, value) in &first.validators.validators {
        let entry = ProcessCpuUsage {
            collective: f32::max(
                value.collective,
                second
                    .validators
                    .validators
                    .get(key)
                    .unwrap_or(&value)
                    .collective,
            ),
            task_threads: max_merge_float_maps(
                value.task_threads.clone(),
                second
                    .validators
                    .validators
                    .get(key)
                    .unwrap_or(&value)
                    .task_threads
                    .clone(),
            ),
        };
        validators_merged.insert(key.clone(), entry);
    }

    for (key, value) in &second.validators.validators {
        let entry = ProcessCpuUsage {
            collective: f32::max(
                value.collective,
                first
                    .validators
                    .validators
                    .get(key)
                    .unwrap_or(&value)
                    .collective,
            ),
            task_threads: max_merge_float_maps(
                value.task_threads.clone(),
                first
                    .validators
                    .validators
                    .get(key)
                    .unwrap_or(&value)
                    .task_threads
                    .clone(),
            ),
        };
        validators_merged.insert(key.clone(), entry);
    }

    let total = f32::max(first.validators.total, second.validators.total);

    let validator_stats = ValidatorCpuStats {
        total,
        validators: validators_merged,
    };

    CpuStats {
        node,
        validators: validator_stats,
    }
}

fn merge_validator_memory_stats(
    first: ValidatorMemoryStats,
    second: ValidatorMemoryStats,
) -> ValidatorMemoryStats {
    let total = cmp::max(first.total, second.total);

    let validators = max_merge_maps(first.validators, second.validators);

    ValidatorMemoryStats { total, validators }
}

fn merge_validator_io_stats(first: ValidatorIOStats, second: ValidatorIOStats) -> ValidatorIOStats {
    let total = DiskReadWrite {
        read_bytes_per_sec: cmp::max(
            first.total.read_bytes_per_sec,
            second.total.read_bytes_per_sec,
        ),
        written_bytes_per_sec: cmp::max(
            first.total.written_bytes_per_sec,
            second.total.written_bytes_per_sec,
        ),
    };

    let mut new_map = HashMap::new();
    for (key, value) in first.validators.iter() {
        new_map.insert(
            key.clone(),
            DiskReadWrite {
                read_bytes_per_sec: cmp::max(
                    value.read_bytes_per_sec,
                    second
                        .validators
                        .get(key)
                        .unwrap_or(&DiskReadWrite::default())
                        .clone()
                        .read_bytes_per_sec,
                ),
                written_bytes_per_sec: cmp::max(
                    value.written_bytes_per_sec,
                    second
                        .validators
                        .get(key)
                        .unwrap_or(&DiskReadWrite::default())
                        .clone()
                        .written_bytes_per_sec,
                ),
            },
        );
    }

    for (key, value) in second.validators.iter() {
        new_map.insert(
            key.clone(),
            DiskReadWrite {
                read_bytes_per_sec: cmp::max(
                    value.read_bytes_per_sec,
                    first
                        .validators
                        .get(key)
                        .unwrap_or(&DiskReadWrite::default())
                        .clone()
                        .read_bytes_per_sec,
                ),
                written_bytes_per_sec: cmp::max(
                    value.written_bytes_per_sec,
                    first
                        .validators
                        .get(key)
                        .unwrap_or(&DiskReadWrite::default())
                        .clone()
                        .written_bytes_per_sec,
                ),
            },
        );
    }

    ValidatorIOStats {
        total,
        validators: new_map,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_info::TezedgeDiskData;
    use itertools::Itertools;

    macro_rules! map(
        { $($key:expr => $value:expr),+ } => {
            {
                let mut m = ::std::collections::HashMap::new();
                $(
                    m.insert($key, $value);
                )+
                m
            }
         };
    );

    #[test]
    fn test_mergable_resources() {
        let node_threads1 = map!("thread1".to_string() => 100.0, "thread2".to_string() => 200.0);
        let validators_threads1 =
            map!("thread1".to_string() => 10.0, "thread2".to_string() => 20.0);
        let validators_cpu1 = map!("val1".to_string() => ProcessCpuUsage {
            collective: 100.0,
            task_threads: validators_threads1
        });

        let validators_memory1 = map!("val1".to_string() => 10050);
        let validators_io1 = map!("val1".to_string() => DiskReadWrite {
            read_bytes_per_sec: 15000,
            written_bytes_per_sec: 25000,
        });

        let resources1 = ResourceUtilization {
            cpu: CpuStats {
                node: ProcessCpuUsage {
                    collective: 150.0,
                    task_threads: node_threads1,
                },
                validators: ValidatorCpuStats {
                    total: 100.0,
                    validators: validators_cpu1,
                },
            },
            tezedge_disk: TezedgeDiskData::new(1, 1, 1, 1, 1, 1).into(),
            ocaml_disk: None,
            memory: MemoryStats {
                node: 100,
                validators: ValidatorMemoryStats {
                    total: 10050,
                    validators: validators_memory1,
                },
            },
            timestamp: 1,
            io: IOStats {
                node: DiskReadWrite {
                    read_bytes_per_sec: 2000,
                    written_bytes_per_sec: 1500,
                },

                validators: ValidatorIOStats {
                    total: DiskReadWrite {
                        read_bytes_per_sec: 15000,
                        written_bytes_per_sec: 25000,
                    },
                    validators: validators_io1,
                },
            },
            head_info: NodeInfo::default(),
            network: NetworkStats {
                received_bytes_per_sec: 1000,
                sent_bytes_per_sec: 5000,
            },
            free_disk_space: 1000,
            total_disk_space: 100000,
        };

        let node_threads2 = map!("thread1".to_string() => 50.0, "thread2".to_string() => 100.0);
        let validators_threads2 = map!("thread1".to_string() => 5.0, "thread2".to_string() => 10.0);
        let validators_cpu2 = map!("val1".to_string() => ProcessCpuUsage {
            collective: 120.0,
            task_threads: validators_threads2
        });

        let validators_memory2 = map!("val1".to_string() => 23050);
        let validators_io2 = map!("val1".to_string() => DiskReadWrite {
            read_bytes_per_sec: 5000,
            written_bytes_per_sec: 35000,
        });

        let resources2 = ResourceUtilization {
            cpu: CpuStats {
                node: ProcessCpuUsage {
                    collective: 250.0,
                    task_threads: node_threads2,
                },
                validators: ValidatorCpuStats {
                    total: 150.0,
                    validators: validators_cpu2,
                },
            },
            tezedge_disk: TezedgeDiskData::new(6, 5, 4, 3, 2, 125).into(),
            ocaml_disk: None,
            memory: MemoryStats {
                node: 10,
                validators: ValidatorMemoryStats {
                    total: 25050,
                    validators: validators_memory2,
                },
            },
            timestamp: 2,
            io: IOStats {
                node: DiskReadWrite {
                    read_bytes_per_sec: 6000,
                    written_bytes_per_sec: 500,
                },
                validators: ValidatorIOStats {
                    total: DiskReadWrite {
                        read_bytes_per_sec: 96000,
                        written_bytes_per_sec: 444000,
                    },
                    validators: validators_io2,
                },
            },
            head_info: NodeInfo::default(),
            network: NetworkStats {
                received_bytes_per_sec: 8000,
                sent_bytes_per_sec: 4000,
            },
            free_disk_space: 900,
            total_disk_space: 100000,
        };

        let node_threads3 = map!("thread1".to_string() => 50.0, "thread2".to_string() => 350.0, "thread3".to_string() => 200.0);
        let validators_threads3 = map!("thread1".to_string() => 5.0, "thread2".to_string() => 35.0, "thread3".to_string() => 20.0);
        let validators_cpu3 = map!("val1".to_string() => ProcessCpuUsage {
            collective: 190.0,
            task_threads: validators_threads3
        });

        let validators_memory3 = map!("val1".to_string() => 2050);
        let validators_io3 = map!("val1".to_string() => DiskReadWrite {
            read_bytes_per_sec: 19000,
            written_bytes_per_sec: 0,
        });

        let resources3 = ResourceUtilization {
            cpu: CpuStats {
                node: ProcessCpuUsage {
                    collective: 150.0,
                    task_threads: node_threads3,
                },
                validators: ValidatorCpuStats {
                    total: 190.0,
                    validators: validators_cpu3,
                },
            },
            tezedge_disk: TezedgeDiskData::new(12, 11, 10, 9, 8, 7).into(),
            ocaml_disk: None,
            memory: MemoryStats {
                node: 1500,
                validators: ValidatorMemoryStats {
                    total: 2050,
                    validators: validators_memory3,
                },
            },
            timestamp: 3,
            io: IOStats {
                node: DiskReadWrite {
                    read_bytes_per_sec: 2000,
                    written_bytes_per_sec: 500,
                },

                validators: ValidatorIOStats {
                    total: DiskReadWrite {
                        read_bytes_per_sec: 96000,
                        written_bytes_per_sec: 25800,
                    },
                    validators: validators_io3,
                },
            },
            head_info: NodeInfo::default(),
            network: NetworkStats {
                received_bytes_per_sec: 100,
                sent_bytes_per_sec: 48,
            },
            free_disk_space: 800,
            total_disk_space: 100000,
        };

        let node_thread_expected = map!("thread1".to_string() => 100.0, "thread2".to_string() => 350.0, "thread3".to_string() => 200.0);
        let validators_threads_expected = map!("thread1".to_string() => 10.0, "thread2".to_string() => 35.0, "thread3".to_string() => 20.0);
        let validators_cpu_expected = map!("val1".to_string() => ProcessCpuUsage {
            collective: 190.0,
            task_threads: validators_threads_expected
        });

        let validators_memory_expected = map!("val1".to_string() => 23050);
        let validators_io_expected = map!("val1".to_string() => DiskReadWrite {
            read_bytes_per_sec: 19000,
            written_bytes_per_sec: 35000,
        });

        let expected = ResourceUtilization {
            cpu: CpuStats {
                node: ProcessCpuUsage {
                    collective: 250.0,
                    task_threads: node_thread_expected,
                },
                validators: ValidatorCpuStats {
                    total: 190.0,
                    validators: validators_cpu_expected,
                },
            },
            tezedge_disk: TezedgeDiskData::new(12, 11, 10, 9, 8, 125).into(),
            ocaml_disk: None,
            memory: MemoryStats {
                node: 1500,
                validators: ValidatorMemoryStats {
                    total: 25050,
                    validators: validators_memory_expected,
                },
            },
            timestamp: 3,
            io: IOStats {
                node: DiskReadWrite {
                    read_bytes_per_sec: 6000,
                    written_bytes_per_sec: 1500,
                },

                validators: ValidatorIOStats {
                    total: DiskReadWrite {
                        read_bytes_per_sec: 96000,
                        written_bytes_per_sec: 444000,
                    },
                    validators: validators_io_expected,
                },
            },
            head_info: NodeInfo::default(),
            network: NetworkStats {
                received_bytes_per_sec: 8000,
                sent_bytes_per_sec: 5000,
            },
            free_disk_space: 800,
            total_disk_space: 100000,
        };

        let resources = vec![resources1, resources2, resources3];
        let merged_final = resources.into_iter().fold1(|m1, m2| m1.merge(m2)).unwrap();

        assert_eq!(merged_final.cpu.node, expected.cpu.node);
        assert_eq!(merged_final.cpu.validators, expected.cpu.validators);
        assert_eq!(merged_final.tezedge_disk, expected.tezedge_disk);
        assert_eq!(merged_final.memory.node, expected.memory.node);
        assert_eq!(merged_final.memory.validators, expected.memory.validators);
        assert_eq!(merged_final.timestamp, expected.timestamp);
        assert_eq!(merged_final.io.node, expected.io.node);
        assert_eq!(merged_final.io.validators, expected.io.validators);
        assert_eq!(merged_final.network, expected.network);
    }
}
