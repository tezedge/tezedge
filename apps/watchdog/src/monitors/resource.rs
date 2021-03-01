// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use crate::monitors::Alerts;
use crate::node::OcamlNode;
use crate::slack::SlackServer;
use chrono::Utc;

use serde::Serialize;
use slog::Logger;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use getset::Getters;
use sysinfo::{System, SystemExt};

use shell::stats::memory::ProcessMemoryStats;

use crate::display_info::DiskData;
use crate::node::{Node, TezedgeNode, OCAML_PORT, TEZEDGE_PORT};

pub type ResourceUtilizationStorage = Arc<RwLock<VecDeque<ResourceUtilization>>>;

/// The max capacity of the VecDeque holding the measurements
pub const MEASUREMENTS_MAX_CAPACITY: usize = 40320;

pub struct ResourceMonitor {
    ocaml_resource_utilization: ResourceUtilizationStorage,
    tezedge_resource_utilization: ResourceUtilizationStorage,
    last_checked_head_level: Option<u64>,
    alerts: Alerts,
    log: Logger,
    slack: SlackServer,
    system: System,
}

#[derive(Clone, Debug, Default, Serialize, Getters)]
pub struct MemoryStats {
    #[get = "pub(crate)"]
    node: ProcessMemoryStats,

    #[get = "pub(crate)"]
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol_runners: Option<ProcessMemoryStats>,

    #[get = "pub(crate)"]
    #[serde(skip_serializing_if = "Option::is_none")]
    validators: Option<ProcessMemoryStats>,
}

#[derive(Clone, Debug, Serialize, Getters)]
pub struct ResourceUtilization {
    #[get = "pub(crate)"]
    timestamp: i64,

    #[get = "pub(crate)"]
    memory: MemoryStats,

    #[get = "pub(crate)"]
    disk: DiskData,

    #[get = "pub(crate)"]
    cpu: CpuStats,
}

#[derive(Clone, Debug, Serialize, Getters)]
pub struct CpuStats {
    #[get = "pub(crate)"]
    node: i32,

    #[get = "pub(crate)"]
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol_runners: Option<i32>,
}

impl ResourceMonitor {
    pub fn new(
        ocaml_resource_utilization: ResourceUtilizationStorage,
        tezedge_resource_utilization: ResourceUtilizationStorage,
        last_checked_head_level: Option<u64>,
        alerts: Alerts,
        log: Logger,
        slack: SlackServer,
    ) -> Self {
        Self {
            ocaml_resource_utilization,
            tezedge_resource_utilization,
            last_checked_head_level,
            alerts,
            log,
            slack,
            system: System::new_all(),
        }
    }

    pub async fn take_measurement(&mut self) -> Result<(), failure::Error> {
        // memory rpc
        let tezedge_node = TezedgeNode::collect_memory_data(TEZEDGE_PORT).await?;
        let ocaml_node = OcamlNode::collect_memory_data(OCAML_PORT).await?;

        // protocol runner memory rpc
        let protocol_runners =
            TezedgeNode::collect_protocol_runners_memory_stats(TEZEDGE_PORT).await?;

        // tezos validators memory data
        let tezos_validators = OcamlNode::collect_validator_memory_stats()?;

        // collect disk stats
        let tezedge_disk = TezedgeNode::collect_disk_data()?;
        let ocaml_disk = OcamlNode::collect_disk_data()?;

        // cpu stats
        self.system.refresh_all();
        let tezedge_cpu = TezedgeNode::collect_cpu_data(&mut self.system, "light-node")?;
        let protocol_runners_cpu =
            TezedgeNode::collect_cpu_data(&mut self.system, "protocol-runner")?;
        let ocaml_cpu = OcamlNode::collect_cpu_data(&mut self.system, "tezos-node")?;

        let tezedge_resources = ResourceUtilization {
            timestamp: chrono::Local::now().timestamp(),
            memory: MemoryStats {
                node: tezedge_node,
                protocol_runners: Some(protocol_runners),
                validators: None,
            },
            disk: tezedge_disk,
            cpu: CpuStats {
                node: tezedge_cpu,
                protocol_runners: Some(protocol_runners_cpu),
            },
        };

        let ocaml_resources = ResourceUtilization {
            timestamp: chrono::Local::now().timestamp(),
            memory: MemoryStats {
                node: ocaml_node,
                protocol_runners: None,
                validators: Some(tezos_validators),
            },
            disk: ocaml_disk,
            cpu: CpuStats {
                node: ocaml_cpu,
                protocol_runners: None,
            },
        };

        // custom block to drop the write lock as soon as possible
        {
            let ocaml_resources_ref = &mut *self.ocaml_resource_utilization.write().unwrap();
            let tezedge_resources_ref = &mut *self.tezedge_resource_utilization.write().unwrap();

            // if we are about to exceed the max capacity, remove the last element in the VecDeque
            if ocaml_resources_ref.len() == MEASUREMENTS_MAX_CAPACITY
                && tezedge_resources_ref.len() == MEASUREMENTS_MAX_CAPACITY
            {
                ocaml_resources_ref.pop_back();
                tezedge_resources_ref.pop_back();
            }

            tezedge_resources_ref.push_front(tezedge_resources.clone());
            ocaml_resources_ref.push_front(ocaml_resources);
        }

        // handle alerts
        self.handle_alerts(tezedge_resources).await?;

        Ok(())
    }

    async fn handle_alerts(
        &mut self,
        last_measurement: ResourceUtilization,
    ) -> Result<(), failure::Error> {
        let ResourceMonitor {
            log,
            last_checked_head_level,
            alerts,
            slack,
            ..
        } = self;

        // current time timestamp
        let current_time = Utc::now().timestamp();

        let current_head_level = *TezedgeNode::collect_head_data(TEZEDGE_PORT)
            .await?
            .level();

        alerts.check_disk_alert(&slack, current_time).await?;
        alerts
            .check_memory_alert(&slack, current_time, last_measurement.clone())
            .await?;
        alerts
            .check_node_stuck_alert(
                last_checked_head_level,
                current_head_level,
                current_time,
                &slack,
                log,
            )
            .await?;

        Ok(())
    }
}
