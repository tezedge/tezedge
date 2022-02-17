// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeSet;

use crate::service::rpc_service::RpcId;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct RpcState {
    pub bootstrapped: Bootstrapped,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Bootstrapped {
    pub requests: BTreeSet<RpcId>,
    pub state: Option<BootstrapState>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BootstrapState {
    pub json: serde_json::Value,
    pub is_bootstrapped: bool,
}
