// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod current_head;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct Stats {
    current_head: current_head::CurrentHeadStats,
}
