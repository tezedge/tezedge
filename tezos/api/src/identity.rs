// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::Deserialize;

/// This node identity information
#[derive(Clone, Debug, Deserialize)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
}