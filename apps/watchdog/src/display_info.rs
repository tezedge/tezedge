// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
use std::fmt;

use serde::Serialize;
use merge::Merge;

use shell::stats::memory::ProcessMemoryStats;

pub struct ImagesInfo {
    node: String,
    debugger: String,
    explorer: String,
}

impl ImagesInfo {
    pub fn new(node: String, debugger: String, explorer: String) -> Self {
        Self {
            node,
            debugger,
            explorer,
        }
    }
}

impl fmt::Display for ImagesInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\nNode image: {}\nDebugger image: {}\nExplorer image: {}",
            self.node, self.debugger, self.explorer,
        )
    }
}

#[derive(Serialize, Debug)]
pub struct DiskSpaceData {
    total_disk_space: u64,
    free_disk_space: u64,
    tezedge: DiskData,
    ocaml: DiskData,
}

impl DiskSpaceData {
    pub fn to_megabytes(&self) -> Self {
        Self::new(
            self.total_disk_space / 1024 / 1024,
            self.free_disk_space / 1024 / 1024,
            self.tezedge.clone(),
            self.ocaml.clone(),
        )
    }

    pub fn new(
        total_disk_space: u64,
        free_disk_space: u64,
        tezedge: DiskData,
        ocaml: DiskData,
    ) -> Self {
        Self {
            total_disk_space,
            free_disk_space,
            tezedge,
            ocaml,
        }
    }
}

impl fmt::Display for DiskSpaceData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f,
            "\nFile system: \n\t Total space: {} MB\n\t Free space: {} MB\n\n{}{}",
            self.total_disk_space,
            self.free_disk_space,
            self.tezedge,
            self.ocaml,
        )
    }
}

#[derive(Serialize, Debug)]
pub struct NodeInfo {
    level: u64,
    block_hash: String,
    timestamp: String,
}

impl NodeInfo {
    pub fn new(level: u64, block_hash: String, timestamp: String) -> Self {
        Self {
            level,
            block_hash,
            timestamp,
        }
    }
}

impl fmt::Display for NodeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\n\tLevel: {}\n\tBlock hash: {}\n\tTimestamp: {}",
            self.level, self.block_hash, self.timestamp,
        )
    }
}

#[derive(Serialize)]
pub struct HeadData {
    ocaml: NodeInfo,
    tezedge: NodeInfo,
}

impl HeadData {
    pub fn new(ocaml: NodeInfo, tezedge: NodeInfo) -> Self {
        Self { ocaml, tezedge }
    }
}

impl fmt::Display for HeadData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\nTezedge node: {}\nOcaml node: {}",
            self.tezedge, self.ocaml,
        )
    }
}

#[derive(Serialize)]
pub struct TezedgeSpecificMemoryData {
    light_node: ProcessMemoryStats,
    protocol_runners: ProcessMemoryStats,
}

impl fmt::Display for TezedgeSpecificMemoryData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut total = ProcessMemoryStats::default();
        total.merge(self.light_node.clone());
        total.merge(self.protocol_runners.clone());
        writeln!(
            f,
            "\n\tLight-node: {}\n\tProtocol runners: {}\n\tTotal: {}",
            self.light_node, self.protocol_runners, total
        )
    }
}

impl TezedgeSpecificMemoryData {
    pub fn new(light_node: ProcessMemoryStats, protocol_runners: ProcessMemoryStats) -> Self {
        Self { light_node, protocol_runners }
    }
}

#[derive(Serialize)]
pub struct MemoryData {
    ocaml: ProcessMemoryStats,
    tezedge: TezedgeSpecificMemoryData,
}

impl fmt::Display for MemoryData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\nTezedge node: {}\nOcaml node: {}",
            self.tezedge, self.ocaml,
        )
    }
}

impl MemoryData {
    pub fn new(ocaml: ProcessMemoryStats, tezedge: TezedgeSpecificMemoryData) -> Self {
        Self { ocaml, tezedge }
    }
}

#[derive(Serialize, PartialEq, Clone, Debug, Default)]
pub struct OcamlDiskData {
    debugger_disk_usage: u64,
    node_disk_usage: u64,
}

impl fmt::Display for OcamlDiskData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let total = self.debugger_disk_usage + self.node_disk_usage;
        write!(
            f,
            "{} MB (total)\n\tDebugger: {} MB\n\tNode: {} MB",
            total, self.debugger_disk_usage, self.node_disk_usage,
        )
    }
}

impl OcamlDiskData {
    pub fn new(debugger_disk_usage: u64, node_disk_usage: u64) -> Self {
        Self {
            debugger_disk_usage,
            node_disk_usage,
        }
    }

    pub fn to_megabytes(&self) -> Self {
        Self {
            debugger_disk_usage: self.debugger_disk_usage / 1024 / 1024,
            node_disk_usage: self.node_disk_usage / 1024 / 1024,
        }
    }
}

#[derive(Serialize, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum DiskData {
    Ocaml(OcamlDiskData),
    Tezedge(TezedgeDiskData),
}

impl From<OcamlDiskData> for DiskData {
    fn from(data: OcamlDiskData) -> Self {
        DiskData::Ocaml(data)
    }
}

impl From<TezedgeDiskData> for DiskData {
    fn from(data: TezedgeDiskData) -> Self {
        DiskData::Tezedge(data)
    }
}

impl fmt::Display for DiskData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiskData::Tezedge(tezedge) => {
                write!(
                    f,
                    "Tezedge node: {}",
                    tezedge.to_megabytes(),
                )
            }
            DiskData::Ocaml(ocaml) => {
                write!(
                    f,
                    "\nOcaml node: {}",
                    ocaml.to_megabytes(),
                )
            }
        }
    }
}

#[derive(Serialize, PartialEq, Clone, Debug, Default)]
pub struct TezedgeDiskData {
    debugger_disk_usage: u64,
    context_irmin: u64,
    context_merkle_rocksdb: u64,
    block_storage: u64,
    context_actions: u64,
    main_db: u64,
}

impl fmt::Display for TezedgeDiskData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let TezedgeDiskData {
            debugger_disk_usage,
            context_actions,
            context_irmin,
            context_merkle_rocksdb,
            block_storage,
            main_db
        } = self;

        let total = debugger_disk_usage + context_actions + context_irmin + context_merkle_rocksdb + block_storage + main_db;
        writeln!(
            f,
            "{} MB (total)\n\tMain database: {} MB\n\tContex - irmin: {} MB\n\tContext - rust_merkel_tree: {} MB\n\tContext actions: {} MB\n\tBlock storage (commit log): {} MB\n\tDebugger: {} MB",
            total,
            main_db,
            context_irmin,
            context_merkle_rocksdb,
            context_actions,
            block_storage,
            debugger_disk_usage,
        )
    }
}

impl TezedgeDiskData {
    pub fn new(debugger_disk_usage: u64, context_irmin: u64, context_merkle_rocksdb: u64, block_storage: u64, context_actions: u64, main_db: u64) -> Self {
        Self {
            debugger_disk_usage,
            context_irmin,
            context_merkle_rocksdb,
            block_storage,
            context_actions,
            main_db,
        }
    }

    pub fn to_megabytes(&self) -> Self {
        Self {
            debugger_disk_usage: self.debugger_disk_usage / 1024 / 1024,
            context_irmin: self.context_irmin / 1024 / 1024,
            context_merkle_rocksdb: self.context_merkle_rocksdb / 1024 / 1024,
            block_storage: self.block_storage / 1024 / 1024,
            context_actions: self.context_actions / 1024 / 1024,
            main_db: self.main_db / 1024 / 1024,
        }
    }
}

#[derive(Serialize)]
pub struct CommitHashes {
    ocaml: String,
    tezedge: String,
    debugger: String,
    explorer: String,
}

impl fmt::Display for CommitHashes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ocaml_repo = "https://gitlab.com/tezos/tezos/-/commit";
        let tezedge_repo = "https://github.com/simplestaking/tezedge/commit";
        let debugger_repo = "https://github.com/simplestaking/tezedge-debugger/commit";
        let exploere_repo = "https://github.com/simplestaking/tezedge-explorer/commit";
        writeln!(
            f,
            "\nOcaml: {}/{}\nTezedge: {}/{}\nDebugger: {}/{}\nExplorer: {}/{}",
            ocaml_repo, self.ocaml, tezedge_repo, self.tezedge, debugger_repo, self.debugger, exploere_repo, self.explorer
        )
    }
}

impl CommitHashes {
    pub fn new(ocaml: String, tezedge: String, debugger: String, explorer: String) -> Self {
        Self {
            ocaml,
            tezedge,
            debugger,
            explorer,
        }
    }
}