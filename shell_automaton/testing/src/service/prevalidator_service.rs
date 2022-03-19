// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub use shell_automaton::{protocol::ProtocolAction, service::PrevalidatorService};

use tezos_api::ffi::{BeginConstructionRequest, ValidateOperationRequest};

#[derive(Debug, Clone)]
pub struct PrevalidatorServiceDummy {}

impl PrevalidatorServiceDummy {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for PrevalidatorServiceDummy {
    fn default() -> Self {
        Self::new()
    }
}

impl PrevalidatorService for PrevalidatorServiceDummy {
    fn try_recv(&mut self) -> Result<ProtocolAction, ()> {
        Err(())
    }

    fn begin_construction_for_prevalidation(&mut self, request: BeginConstructionRequest) {
        let _ = request;
    }

    fn validate_operation_for_prevalidation(&mut self, request: ValidateOperationRequest) {
        let _ = request;
    }

    fn begin_construction_for_mempool(&mut self, request: BeginConstructionRequest) {
        let _ = request;
    }

    fn validate_operation_for_mempool(&mut self, request: ValidateOperationRequest) {
        let _ = request;
    }
}
