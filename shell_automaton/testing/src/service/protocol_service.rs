// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use shell_automaton::{service::ProtocolService, protocol::ProtocolAction};

use tezos_api::ffi::BeginConstructionRequest;

#[derive(Debug, Clone)]
pub struct ProtocolServiceDummy {}

impl ProtocolServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl ProtocolService for ProtocolServiceDummy {
    fn try_recv(&mut self) -> Result<ProtocolAction, ()> {
        Err(())
    }

    fn init_protocol_for_read(&mut self) {}

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest) {
        let _ = request;
    }
}
