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

#[derive(Serialize, Debug, Default)]
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
            self.tezedge.to_megabytes(),
            self.ocaml.to_megabytes(),
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
            "\nFile system: \n\t Total space: {} MB\n\t Free space: {} MB\n\nTezedge node:\n\t Debugger: {} MB\n\t Node: {} MB\n\nOcaml node:\n\t Debugger: {} MB\n\t Node: {} MB",
            self.total_disk_space,
            self.free_disk_space,
            self.tezedge.debugger_disk_usage,
            self.tezedge.node_disk_usage,
            self.ocaml.debugger_disk_usage,
            self.ocaml.node_disk_usage,
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

#[derive(Serialize, Debug, Default)]
pub struct DiskData {
    debugger_disk_usage: u64,
    node_disk_usage: u64,
}

impl fmt::Display for DiskData {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\nDebugger: {}\nNode: {}",
            self.debugger_disk_usage, self.node_disk_usage,
        )
    }
}

impl DiskData {
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