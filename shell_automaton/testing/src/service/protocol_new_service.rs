// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use shell_automaton::service::ProtocolNewService;

#[derive(Debug, Clone)]
pub struct ProtocolNewServiceDummy {}

impl ProtocolNewServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl ProtocolNewService for ProtocolNewServiceDummy {
    fn spawn_process(&mut self, name: &str) {
        let _ = name;
    }
}
