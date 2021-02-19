// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use crate::node::OcamlNode;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};

use serde::Serialize;
use slog::Logger;

use shell::stats::memory::ProcessMemoryStats;

use crate::display_info::DiskData;
use crate::node::{Node, TezedgeNode, OCAML_PORT, TEZEDGE_PORT};

pub type ResourceUtilizationStorage = Arc<RwLock<VecDeque<ResourceUtilization>>>;

/// The max capacity of the VecDeque holding the measurements
pub const MEASUREMENTS_MAX_CAPACITY: usize = 1440;

#[derive(Clone, Debug)]
pub struct ResourceMonitor {
    ocaml_resource_utilization: ResourceUtilizationStorage,
    tezedge_resource_utilization: ResourceUtilizationStorage,
    log: Logger,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct MemoryStats {
    node: ProcessMemoryStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol_runners: Option<ProcessMemoryStats>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ResourceUtilization {
    timestamp: i64,
    memory: MemoryStats,
    disk: DiskData,
    cpu: CpuStats,
}

#[derive(Clone, Debug, Serialize)]
pub struct CpuStats {
    node: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    protocol_runners: Option<i32>,
}

impl ResourceMonitor {
    pub fn new(
        ocaml_resource_utilization: ResourceUtilizationStorage,
        tezedge_resource_utilization: ResourceUtilizationStorage,
        log: Logger,
    ) -> Self {
        Self {
            ocaml_resource_utilization,
            tezedge_resource_utilization,
            log,
        }
    }

    pub async fn take_measurement(&self) -> Result<(), failure::Error> {
        // memory rpc
        let tezedge_node = TezedgeNode::collect_memory_data(&self.log, TEZEDGE_PORT).await?;
        let ocaml_node = OcamlNode::collect_memory_data(&self.log, OCAML_PORT).await?;

        // protocol runner memory rpc
        let protocol_runners =
            TezedgeNode::collect_protocol_runners_memory_stats(TEZEDGE_PORT).await?;

        // collect disk stats
        let tezedge_disk = TezedgeNode::collect_disk_data()?;
        let ocaml_disk = OcamlNode::collect_disk_data()?;

        // cpu stats
        let tezedge_cpu = TezedgeNode::collect_cpu_data("light-node")?;
        let protocol_runners_cpu = TezedgeNode::collect_cpu_data("protocol-runner")?;
        let ocaml_cpu = OcamlNode::collect_cpu_data("tezos-node")?;

        let ocaml_resources_ref = &mut *self.ocaml_resource_utilization.write().unwrap();
        let tezedge_resources_ref = &mut *self.tezedge_resource_utilization.write().unwrap();

        // if we are about to exceed the max capacity, remove the last element in the VecDeque
        if ocaml_resources_ref.len() == MEASUREMENTS_MAX_CAPACITY
            && tezedge_resources_ref.len() == MEASUREMENTS_MAX_CAPACITY
        {
            ocaml_resources_ref.pop_back();
            tezedge_resources_ref.pop_back();
        }
        tezedge_resources_ref.push_front(ResourceUtilization {
            timestamp: chrono::Local::now().timestamp(),
            memory: MemoryStats {
                node: tezedge_node,
                protocol_runners: Some(protocol_runners),
            },
            disk: tezedge_disk,
            cpu: CpuStats {
                node: tezedge_cpu,
                protocol_runners: Some(protocol_runners_cpu),
            },
        });
        ocaml_resources_ref.push_front(ResourceUtilization {
            timestamp: chrono::Local::now().timestamp(),
            memory: MemoryStats {
                node: ocaml_node,
                protocol_runners: None,
            },
            disk: ocaml_disk,
            cpu: CpuStats {
                node: ocaml_cpu,
                protocol_runners: None,
            },
        });

        Ok(())
    }
}
