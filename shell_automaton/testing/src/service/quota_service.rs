// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use shell_automaton::service::QuotaService;

#[derive(Debug, Clone)]
pub struct QuotaServiceDummy {}

impl QuotaServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl QuotaService for QuotaServiceDummy {}
