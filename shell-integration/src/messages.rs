// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize, Debug)]
pub struct WorkerStatus {
    pub phase: WorkerStatusPhase,
    pub since: String,
}

#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum WorkerStatusPhase {
    #[serde(rename = "running")]
    Running,
}

#[derive(Serialize, Debug)]
pub struct Prevalidator {
    pub chain_id: String,
    pub status: WorkerStatus,
    // TODO: implement the json structure form ocaml's RPC
    // TODO: missing Tezos fields
    // information
    // pipelines
}
