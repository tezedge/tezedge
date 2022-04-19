// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use crypto::hash::ContextHash;
use serde::{Deserialize, Serialize};

use slog::Logger;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::{
    ApplyBlockRequest, ApplyBlockResponse, BeginConstructionRequest, CommitGenesisResult,
    InitProtocolContextResult, PrevalidatorWrapper, TezosRuntimeConfiguration,
    ValidateOperationRequest, ValidateOperationResponse,
};
use tezos_context_api::{PatchContext, TezosContextStorageConfiguration};
use tezos_protocol_ipc_client::{ProtocolRunnerApi, ProtocolRunnerError, ProtocolServiceError};
use tezos_protocol_ipc_messages::{
    GenesisResultDataParams, InitProtocolContextParams, ProtocolMessage,
};

use crate::protocol_runner::ProtocolRunnerToken;

use super::protocol_runner_service_worker::ProtocolRunnerServiceWorker;
use super::service_async_channel::{
    worker_channel, ResponseTryRecvError, ServiceWorkerAsyncRequester,
};

pub type ProtocolRunnerResponse = ProtocolRunnerResult;

#[derive(Debug)]
pub enum ProtocolRunnerRequest {
    SpawnServer(()),
    ShutdownServer(()),
    Message((ProtocolRunnerToken, ProtocolMessage)),
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ProtocolRunnerResult {
    SpawnServer(Result<(), ProtocolRunnerError>),
    InitRuntime((ProtocolRunnerToken, Result<(), ProtocolServiceError>)),
    InitContext(
        (
            ProtocolRunnerToken,
            Result<InitProtocolContextResult, ProtocolServiceError>,
        ),
    ),
    InitContextIpcServer((ProtocolRunnerToken, Result<(), ProtocolServiceError>)),

    GetCurrentHead(
        (
            ProtocolRunnerToken,
            Result<Vec<ContextHash>, ProtocolServiceError>,
        ),
    ),

    GenesisCommitResultGet(
        (
            ProtocolRunnerToken,
            Result<CommitGenesisResult, ProtocolServiceError>,
        ),
    ),

    ApplyBlock(
        (
            ProtocolRunnerToken,
            Result<ApplyBlockResponse, ProtocolServiceError>,
        ),
    ),

    BeginConstruction(
        (
            ProtocolRunnerToken,
            Result<PrevalidatorWrapper, ProtocolServiceError>,
        ),
    ),

    ValidateOperation(
        (
            ProtocolRunnerToken,
            Result<ValidateOperationResponse, ProtocolServiceError>,
        ),
    ),

    ShutdownServer(Result<(), ProtocolRunnerError>),
}

impl ProtocolRunnerResult {
    pub fn token(&self) -> Option<ProtocolRunnerToken> {
        match self {
            Self::SpawnServer(_) => None,
            Self::InitRuntime((token, _)) => Some(*token),
            Self::InitContext((token, _)) => Some(*token),
            Self::InitContextIpcServer((token, _)) => Some(*token),
            Self::GetCurrentHead((token, _)) => Some(*token),
            Self::GenesisCommitResultGet((token, _)) => Some(*token),
            Self::ApplyBlock((token, _)) => Some(*token),
            Self::BeginConstruction((token, _)) => Some(*token),
            Self::ValidateOperation((token, _)) => Some(*token),

            Self::ShutdownServer(_) => None,
        }
    }
}

pub type ProtocolRunnerRequester =
    ServiceWorkerAsyncRequester<ProtocolRunnerRequest, ProtocolRunnerResponse>;

pub trait ProtocolRunnerService {
    /// Try to receive/read queued message, if there is any.
    fn try_recv(&mut self) -> Result<ProtocolRunnerResponse, ResponseTryRecvError>;

    fn spawn_server(&mut self);

    fn init_runtime(&mut self, config: TezosRuntimeConfiguration) -> ProtocolRunnerToken;

    fn init_context(
        &mut self,
        storage: TezosContextStorageConfiguration,
        tezos_environment: &TezosEnvironmentConfiguration,
        commit_genesis: bool,
        enable_testchain: bool,
        readonly: bool,
        patch_context: Option<PatchContext>,
        context_stats_db_path: Option<PathBuf>,
    ) -> Result<ProtocolRunnerToken, ProtocolServiceError>;

    fn init_context_ipc_server(
        &mut self,
        cfg: TezosContextStorageConfiguration,
    ) -> ProtocolRunnerToken;

    fn genesis_commit_result_get_init(
        &mut self,
        params: GenesisResultDataParams,
    ) -> ProtocolRunnerToken;

    fn apply_block(&mut self, req: ApplyBlockRequest);

    fn get_latest_context_hashes(&mut self, count: i64) -> ProtocolRunnerToken;

    // Prevalidator
    fn begin_construction_for_prevalidation(
        &mut self,
        req: BeginConstructionRequest,
    ) -> ProtocolRunnerToken;

    fn validate_operation_for_prevalidation(
        &mut self,
        req: ValidateOperationRequest,
    ) -> ProtocolRunnerToken;

    fn begin_construction_for_mempool(
        &mut self,
        req: BeginConstructionRequest,
    ) -> ProtocolRunnerToken;

    fn validate_operation_for_mempool(
        &mut self,
        req: ValidateOperationRequest,
    ) -> ProtocolRunnerToken;

    /// Notify status of protocol runner's and it's context initialization.
    fn notify_status(&mut self, initialized: bool);

    fn shutdown(&mut self);
}

pub struct ProtocolRunnerServiceDefault {
    channel: ProtocolRunnerRequester,
    status_sender: tokio::sync::watch::Sender<bool>,
    connections: slab::Slab<()>,
}

impl ProtocolRunnerServiceDefault {
    pub fn new(
        api: ProtocolRunnerApi,
        mio_waker: Arc<mio::Waker>,
        bound: usize,
        status_sender: tokio::sync::watch::Sender<bool>,
        log: Logger,
    ) -> Self {
        let (c1, c2) = worker_channel(mio_waker, bound);
        let worker = ProtocolRunnerServiceWorker::new(api, c2, log);
        thread::spawn(move || worker.run());

        Self {
            channel: c1,
            status_sender,
            connections: Default::default(),
        }
    }

    fn new_token(&mut self) -> ProtocolRunnerToken {
        ProtocolRunnerToken::new_unchecked(self.connections.insert(()))
    }
}

impl ProtocolRunnerService for ProtocolRunnerServiceDefault {
    #[inline(always)]
    fn try_recv(&mut self) -> Result<ProtocolRunnerResponse, ResponseTryRecvError> {
        let resp = self.channel.try_recv()?;
        if let Some(token) = resp.token() {
            self.connections.remove(token.into());
        }
        Ok(resp)
    }

    fn spawn_server(&mut self) {
        let message = ProtocolRunnerRequest::SpawnServer(());
        self.channel.blocking_send(message).unwrap();
    }

    fn init_runtime(&mut self, config: TezosRuntimeConfiguration) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::ChangeRuntimeConfigurationCall(config);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn init_context(
        &mut self,
        storage: TezosContextStorageConfiguration,
        tezos_environment: &TezosEnvironmentConfiguration,
        commit_genesis: bool,
        enable_testchain: bool,
        readonly: bool,
        patch_context: Option<PatchContext>,
        context_stats_db_path: Option<PathBuf>,
    ) -> Result<ProtocolRunnerToken, ProtocolServiceError> {
        let params = InitProtocolContextParams {
            storage,
            genesis: tezos_environment.genesis.clone(),
            genesis_max_operations_ttl: tezos_environment
                .genesis_additional_data()
                .map_err(|error| ProtocolServiceError::InvalidDataError {
                    message: format!("{:?}", error),
                })?
                .max_operations_ttl,
            protocol_overrides: tezos_environment.protocol_overrides.clone(),
            commit_genesis,
            enable_testchain,
            readonly,
            patch_context,
            context_stats_db_path,
        };

        let token = self.new_token();

        let message = ProtocolMessage::InitProtocolContextCall(params);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();

        Ok(token)
    }

    fn init_context_ipc_server(
        &mut self,
        cfg: TezosContextStorageConfiguration,
    ) -> ProtocolRunnerToken {
        let token = self.new_token();

        let message = ProtocolMessage::InitProtocolContextIpcServer(cfg);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();

        token
    }

    fn genesis_commit_result_get_init(
        &mut self,
        params: GenesisResultDataParams,
    ) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::GenesisResultDataCall(params);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn apply_block(&mut self, req: ApplyBlockRequest) {
        let token = self.new_token();
        let message = ProtocolMessage::ApplyBlockCall(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
    }

    fn begin_construction_for_prevalidation(
        &mut self,
        req: BeginConstructionRequest,
    ) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::BeginConstructionForPrevalidationCall(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn validate_operation_for_prevalidation(
        &mut self,
        req: ValidateOperationRequest,
    ) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::ValidateOperationForPrevalidationCall(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn begin_construction_for_mempool(
        &mut self,
        req: BeginConstructionRequest,
    ) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::BeginConstructionForMempoolCall(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn validate_operation_for_mempool(
        &mut self,
        req: ValidateOperationRequest,
    ) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::ValidateOperationForMempoolCall(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn notify_status(&mut self, initialized: bool) {
        let _ = self.status_sender.send(initialized);
    }

    fn shutdown(&mut self) {
        self.channel
            .blocking_send(ProtocolRunnerRequest::ShutdownServer(()))
            .unwrap();
    }

    fn get_latest_context_hashes(&mut self, count: i64) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::ContextGetLatestContextHashes(count);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }
}
