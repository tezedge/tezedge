// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use serde::Serialize;

use crypto::hash::ChainId;
use storage::BlockHeaderWithHash;

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

/// Module which holds all dedicated struct/enums for notifications used by notifiers
pub mod notifications {

    use super::*;

    #[derive(Debug)]
    pub struct NewCurrentHeadNotification {
        pub chain_id: Arc<ChainId>,
        pub block: Arc<BlockHeaderWithHash>,
        pub is_bootstrapped: bool,
        pub best_remote_level: Option<i32>,
    }

    impl NewCurrentHeadNotification {
        pub fn new(
            chain_id: Arc<ChainId>,
            block: Arc<BlockHeaderWithHash>,
            is_bootstrapped: bool,
            best_remote_level: Option<i32>,
        ) -> Self {
            Self {
                chain_id,
                block,
                is_bootstrapped,
                best_remote_level,
            }
        }
    }

    pub type NewCurrentHeadNotificationRef = Arc<NewCurrentHeadNotification>;
}
