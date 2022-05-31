// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;

use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::{
    ApplyBlockRequest, BeginConstructionRequest, ProtocolRpcRequest, TezosRuntimeConfiguration,
    ValidateOperationRequest,
};
use tezos_context_api::{PatchContext, TezosContextStorageConfiguration};
use tezos_protocol_ipc_client::ProtocolServiceError;
use tezos_protocol_ipc_messages::GenesisResultDataParams;

use shell_automaton::protocol_runner::ProtocolRunnerToken;
pub use shell_automaton::service::protocol_runner_service::{
    ProtocolRunnerResponse, ProtocolRunnerService,
};
use shell_automaton::service::service_async_channel::ResponseTryRecvError;

#[derive(Debug, Clone)]
pub struct ProtocolRunnerServiceDummy {
    connections: slab::Slab<()>,
}

impl ProtocolRunnerServiceDummy {
    pub fn new() -> Self {
        Self {
            connections: Default::default(),
        }
    }

    fn new_token(&mut self) -> ProtocolRunnerToken {
        ProtocolRunnerToken::new_unchecked(self.connections.insert(()))
    }
}

impl Default for ProtocolRunnerServiceDummy {
    fn default() -> Self {
        Self::new()
    }
}

impl ProtocolRunnerService for ProtocolRunnerServiceDummy {
    fn try_recv(&mut self) -> Result<ProtocolRunnerResponse, ResponseTryRecvError> {
        Err(ResponseTryRecvError::Empty)
    }

    fn spawn_server(&mut self) {}

    fn init_runtime(&mut self, _: TezosRuntimeConfiguration) -> ProtocolRunnerToken {
        self.new_token()
    }

    fn init_context(
        &mut self,
        _: TezosContextStorageConfiguration,
        _: &TezosEnvironmentConfiguration,
        _: bool,
        _: bool,
        _: bool,
        _: Option<PatchContext>,
        _: Option<PathBuf>,
    ) -> Result<ProtocolRunnerToken, ProtocolServiceError> {
        Ok(self.new_token())
    }

    fn init_context_ipc_server(
        &mut self,
        _: TezosContextStorageConfiguration,
    ) -> ProtocolRunnerToken {
        self.new_token()
    }

    fn genesis_commit_result_get_init(
        &mut self,
        _: GenesisResultDataParams,
    ) -> ProtocolRunnerToken {
        self.new_token()
    }

    fn apply_block(&mut self, _: ApplyBlockRequest) {}

    fn begin_construction(&mut self, _: BeginConstructionRequest) -> ProtocolRunnerToken {
        self.new_token()
    }

    fn validate_operation(&mut self, _: ValidateOperationRequest) -> ProtocolRunnerToken {
        self.new_token()
    }

    fn get_context_raw_bytes(&mut self, _: ProtocolRpcRequest) -> ProtocolRunnerToken {
        self.new_token()
    }

    fn get_endorsing_rights(&mut self, _: ProtocolRpcRequest) -> ProtocolRunnerToken {
        self.new_token()
    }

    fn get_cycle_delegates(&mut self, _: ProtocolRpcRequest) -> ProtocolRunnerToken {
        self.new_token()
    }

    /// Notify status of protocol runner's and it's context initialization.
    fn notify_status(&mut self, _: bool) {}

    fn shutdown(&mut self) {}

    fn get_latest_context_hashes(&mut self, _: i64) -> ProtocolRunnerToken {
        self.new_token()
    }
}
