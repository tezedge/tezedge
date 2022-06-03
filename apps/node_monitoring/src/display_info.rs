// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::convert::TryFrom;
use std::fmt;

use getset::{CopyGetters, Getters};
use serde::Serialize;

use crate::monitors::resource::ResourceMonitorError;

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DiskSpaceData {
    total_disk_space: u64,
    free_disk_space: u64,
    tezedge: DiskData,
    ocaml: DiskData,
}

impl fmt::Display for DiskSpaceData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\nFile system: \n\t Total space: {} MB\n\t Free space: {} MB\n\n{}{}",
            self.total_disk_space, self.free_disk_space, self.tezedge, self.ocaml,
        )
    }
}

#[derive(Clone, Serialize, Debug, Getters, CopyGetters, Eq, PartialEq, Default)]
pub struct NodeInfo {
    #[get = "pub(crate)"]
    level: u64,

    #[get = "pub(crate)"]
    payload_round: u64,

    #[get = "pub(crate)"]
    block_hash: String,

    #[get = "pub(crate)"]
    timestamp: String,

    #[get = "pub(crate)"]
    protocol: u64,

    #[get = "pub(crate)"]
    cycle_position: u64,

    #[get = "pub(crate)"]
    voting_period_position: u64,

    #[get = "pub(crate)"]
    voting_period: String,
}

impl NodeInfo {
    pub fn new(
        level: u64,
        payload_round: u64,
        block_hash: String,
        timestamp: String,
        protocol: u64,
        cycle_position: u64,
        voting_period_position: u64,
        voting_period: String,
    ) -> Self {
        Self {
            level,
            payload_round,
            block_hash,
            timestamp,
            protocol,
            cycle_position,
            voting_period,
            voting_period_position,
        }
    }
}

impl fmt::Display for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\n\tLevel: {}\n\tPaylaod round: {}\n\tCycle position: {}\n\tProtocol: {}\n\tVoting period: {}\n\tVoting period position: {}\n\tTimestamp: {}",
            self.level, self.payload_round, self.cycle_position, self.protocol, self.voting_period, self.voting_period_position, self.timestamp
        )
    }
}

#[derive(Serialize, PartialEq, Clone, Debug, Default, CopyGetters)]
#[serde(rename_all = "camelCase")]
pub struct OCamlDiskData {
    #[get_copy = "pub(crate)"]
    debugger: u64,

    #[get_copy = "pub(crate)"]
    block_storage: u64,

    #[get_copy = "pub(crate)"]
    context_storage: u64,
}

impl fmt::Display for OCamlDiskData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = self.block_storage + self.context_storage;
        write!(
            f,
            "{} MB (total)\n\tBlock storage: {} MB\n\tContext: {} MB",
            total, self.block_storage, self.context_storage,
        )
    }
}

impl OCamlDiskData {
    pub fn new(debugger: u64, block_storage: u64, context_storage: u64) -> Self {
        Self {
            block_storage,
            context_storage,
            debugger,
        }
    }

    pub fn to_megabytes(&self) -> Self {
        Self {
            block_storage: self.block_storage / 1024 / 1024,
            context_storage: self.context_storage / 1024 / 1024,
            debugger: self.debugger / 1024 / 1024,
        }
    }
}

#[derive(Serialize, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum DiskData {
    OCaml(OCamlDiskData),
    Tezedge(TezedgeDiskData),
}

impl From<OCamlDiskData> for DiskData {
    fn from(data: OCamlDiskData) -> Self {
        DiskData::OCaml(data)
    }
}

impl From<TezedgeDiskData> for DiskData {
    fn from(data: TezedgeDiskData) -> Self {
        DiskData::Tezedge(data)
    }
}

impl TryFrom<DiskData> for TezedgeDiskData {
    type Error = ResourceMonitorError;

    fn try_from(data: DiskData) -> Result<Self, ResourceMonitorError> {
        if let DiskData::Tezedge(disk) = data {
            Ok(disk)
        } else {
            Err(ResourceMonitorError::DiskInfoError {
                reason: "Cannot conver to TezedgeDiskData".to_string(),
            })
        }
    }
}

impl TryFrom<DiskData> for OCamlDiskData {
    type Error = ResourceMonitorError;

    fn try_from(data: DiskData) -> Result<Self, ResourceMonitorError> {
        if let DiskData::OCaml(disk) = data {
            Ok(disk)
        } else {
            Err(ResourceMonitorError::DiskInfoError {
                reason: "Cannot conver to OCamlDiskData".to_string(),
            })
        }
    }
}

impl fmt::Display for DiskData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiskData::Tezedge(tezedge) => {
                write!(f, "Tezedge node: {}", tezedge.to_megabytes(),)
            }
            DiskData::OCaml(ocaml) => {
                write!(f, "\nOCaml node: {}", ocaml.to_megabytes(),)
            }
        }
    }
}

#[derive(Serialize, PartialEq, Clone, Debug, Default, CopyGetters)]
#[serde(rename_all = "camelCase")]
pub struct TezedgeDiskData {
    #[get_copy = "pub(crate)"]
    context_storage: u64,

    #[get_copy = "pub(crate)"]
    block_storage: u64,

    #[get_copy = "pub(crate)"]
    context_stats: u64,

    #[get_copy = "pub(crate)"]
    main_db: u64,

    #[get_copy = "pub(crate)"]
    debugger: u64,
}

impl fmt::Display for TezedgeDiskData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let TezedgeDiskData {
            context_stats,
            context_storage,
            block_storage,
            main_db,
            debugger,
        } = self;

        let total = context_stats + context_storage + block_storage + main_db;
        writeln!(
            f,
            "{} MB (total)\n\tMain database: {} MB\n\tContext: {} MB\n\tContext stats: {} MB\n\tBlock storage (commit log): {} MB\n\tDebugger: {} MB",
            total,
            main_db,
            context_storage,
            context_stats,
            block_storage,
            debugger
        )
    }
}

impl TezedgeDiskData {
    pub fn new(
        debugger: u64,
        context_storage: u64,
        block_storage: u64,
        context_stats: u64,
        main_db: u64,
    ) -> Self {
        Self {
            context_storage,
            block_storage,
            context_stats,
            main_db,
            debugger,
        }
    }

    pub fn to_megabytes(&self) -> Self {
        Self {
            context_storage: self.context_storage / 1024 / 1024,
            block_storage: self.block_storage / 1024 / 1024,
            context_stats: self.context_stats / 1024 / 1024,
            main_db: self.main_db / 1024 / 1024,
            debugger: self.debugger / 1024 / 1024,
        }
    }
}

#[derive(Serialize)]
pub struct CpuData {
    ocaml: i32,
    tezedge: i32,
    protocol_runners: i32,
}

impl fmt::Display for CpuData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\nOCaml: {}%\nTezedge: \n\tNode: {}\n\tProtocol: {}",
            self.ocaml, self.tezedge, self.protocol_runners
        )
    }
}
