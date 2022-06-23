// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use crypto::hash::{ChainId, ContextHash};
use crypto::PublicKeyWithHash;
use serde::{Deserialize, Serialize};

use slog::Logger;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::{
    ApplyBlockRequest, ApplyBlockResponse, BeginConstructionRequest, CommitGenesisResult,
    ComputePathRequest, ComputePathResponse, InitProtocolContextResult, PreapplyBlockRequest,
    PreapplyBlockResponse, PrevalidatorWrapper, ProtocolRpcRequest, ProtocolRpcResponse,
    TezosRuntimeConfiguration, ValidateOperationRequest, ValidateOperationResponse,
};
use tezos_context_api::{PatchContext, TezosContextStorageConfiguration};
use tezos_messages::base::signature_public_key::{SignaturePublicKey, SignaturePublicKeyHash};
use tezos_messages::base::ConversionError;
use tezos_messages::p2p::encoding::block_header::{BlockHeader, Level};
use tezos_protocol_ipc_client::{ProtocolRunnerApi, ProtocolRunnerError, ProtocolServiceError};
use tezos_protocol_ipc_messages::{
    GenesisResultDataParams, InitProtocolContextParams, ProtocolMessage,
};

use crate::protocol_runner::ProtocolRunnerToken;
use crate::rights::{EndorsingPower, Slot};

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

    GenesisCommitResultGet(
        (
            ProtocolRunnerToken,
            Result<CommitGenesisResult, ProtocolServiceError>,
        ),
    ),

    LatestContextHashesGet(
        (
            ProtocolRunnerToken,
            Result<Vec<ContextHash>, ProtocolServiceError>,
        ),
    ),

    PreapplyBlock(
        (
            ProtocolRunnerToken,
            Result<PreapplyBlockResponse, ProtocolServiceError>,
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

    GetContextRawBytes(
        (
            ProtocolRunnerToken,
            Result<Result<Vec<u8>, ContextRawBytesError>, ProtocolServiceError>,
        ),
    ),

    GetEndorsingRights(
        (
            ProtocolRunnerToken,
            Result<EndorsingRightsResponse, ProtocolServiceError>,
        ),
    ),

    GetValidators(
        (
            ProtocolRunnerToken,
            Result<ValidatorsResponse, ProtocolServiceError>,
        ),
    ),

    GetCycleDelegates(
        (
            ProtocolRunnerToken,
            Result<CycleDelegatesResponse, ProtocolServiceError>,
        ),
    ),

    ComputeOperationsPaths(
        (
            ProtocolRunnerToken,
            Result<ComputePathResponse, ProtocolServiceError>,
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
            Self::GenesisCommitResultGet((token, _)) => Some(*token),
            Self::LatestContextHashesGet((token, _)) => Some(*token),
            Self::PreapplyBlock((token, _)) => Some(*token),
            Self::ApplyBlock((token, _)) => Some(*token),
            Self::BeginConstruction((token, _)) => Some(*token),
            Self::ValidateOperation((token, _)) => Some(*token),
            Self::GetContextRawBytes((token, _)) => Some(*token),
            Self::GetEndorsingRights((token, _)) => Some(*token),
            Self::GetValidators((token, _)) => Some(*token),
            Self::GetCycleDelegates((token, _)) => Some(*token),
            Self::ComputeOperationsPaths((token, _)) => Some(*token),

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

    fn get_latest_context_hashes(&mut self, count: i64) -> ProtocolRunnerToken;

    fn preapply_block(&mut self, req: PreapplyBlockRequest) -> ProtocolRunnerToken;

    fn apply_block(&mut self, req: ApplyBlockRequest);

    // Prevalidator
    fn begin_construction(&mut self, req: BeginConstructionRequest) -> ProtocolRunnerToken;

    // TODO: pre_filter_operation

    fn validate_operation(&mut self, req: ValidateOperationRequest) -> ProtocolRunnerToken;

    fn get_context_raw_bytes(&mut self, req: ProtocolRpcRequest) -> ProtocolRunnerToken;

    fn get_endorsing_rights(&mut self, req: ProtocolRpcRequest) -> ProtocolRunnerToken;

    fn get_validators(
        &mut self,
        chain_id: ChainId,
        block_header: BlockHeader,
        level: Level,
    ) -> ProtocolRunnerToken;

    fn get_cycle_delegates(&mut self, req: ProtocolRpcRequest) -> ProtocolRunnerToken;

    fn compute_operations_paths(&mut self, req: ComputePathRequest) -> ProtocolRunnerToken;

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

    fn get_latest_context_hashes(&mut self, count: i64) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::ContextGetLatestContextHashes(count);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn preapply_block(&mut self, req: PreapplyBlockRequest) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::PreapplyBlock(req);
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

    fn begin_construction(&mut self, req: BeginConstructionRequest) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::BeginConstruction(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn validate_operation(&mut self, req: ValidateOperationRequest) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::ValidateOperation(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn get_context_raw_bytes(&mut self, req: ProtocolRpcRequest) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::GetContextRawBytes(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn get_endorsing_rights(&mut self, req: ProtocolRpcRequest) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::GetEndorsingRights(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn get_validators(
        &mut self,
        chain_id: ChainId,
        block_header: BlockHeader,
        level: Level,
    ) -> ProtocolRunnerToken {
        let req = ProtocolRpcRequest {
            block_header,
            chain_arg: "main".to_string(),
            chain_id,
            request: tezos_api::ffi::RpcRequest {
                body: String::new(),
                accept: None,
                content_type: None,
                context_path: format!("/chains/main/blocks/head/helpers/validators?level={level}"),
                meth: tezos_api::ffi::RpcMethod::GET,
            },
        };
        let token = self.new_token();
        let message = ProtocolMessage::GetValidators(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn get_cycle_delegates(&mut self, req: ProtocolRpcRequest) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::GetCycleDelegates(req);
        self.channel
            .blocking_send(ProtocolRunnerRequest::Message((token, message)))
            .unwrap();
        token
    }

    fn compute_operations_paths(&mut self, req: ComputePathRequest) -> ProtocolRunnerToken {
        let token = self.new_token();
        let message = ProtocolMessage::ComputePathCall(req);
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
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[error("error getting endorsing rights: {0}")]
pub struct ProtocolRpcResponseError(String);

impl From<ProtocolRpcResponse> for ProtocolRpcResponseError {
    fn from(source: ProtocolRpcResponse) -> Self {
        Self(match source {
            ProtocolRpcResponse::RPCOk(_) => "ok_should_not_happen".into(),
            ProtocolRpcResponse::RPCConflict(_) => "conflict".into(),
            ProtocolRpcResponse::RPCCreated(_) => "created".into(),
            ProtocolRpcResponse::RPCError(_) => "error".into(),
            ProtocolRpcResponse::RPCForbidden(_) => "forbidden".into(),
            ProtocolRpcResponse::RPCGone(_) => "gone".into(),
            ProtocolRpcResponse::RPCNoContent => "no_content".into(),
            ProtocolRpcResponse::RPCNotFound(_) => "not_found".into(),
            ProtocolRpcResponse::RPCUnauthorized => "unauthorized".into(),
        })
    }
}

#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum ContextRawBytesError {
    #[error(transparent)]
    Response(#[from] ProtocolRpcResponseError),
    #[error("Error decoding bytes from JSON: {0}")]
    JsonDecode(String),
    #[error("Error decoding bytes from hex: {0}")]
    HexDecode(String),
}

pub(super) fn context_raw_bytes_from_rpc_response(
    res: ProtocolRpcResponse,
) -> Result<Vec<u8>, ContextRawBytesError> {
    let body = res.ok_body_or().map_err(ProtocolRpcResponseError::from)?;
    let json_str = serde_json::from_str::<String>(&body)
        .map_err(|err| ContextRawBytesError::JsonDecode(err.to_string()))?;
    let bytes =
        hex::decode(&json_str).map_err(|err| ContextRawBytesError::HexDecode(err.to_string()))?;
    Ok(bytes)
}

pub type EndorsingRightsResponse = Result<EndorsingRights, EndorsingRightsError>;

pub type EndorsingRights = Vec<EndorsingRightsLevel>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct EndorsingRightsLevel {
    pub level: i32,
    pub delegates: Vec<EndorsingRightsDelegate>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct EndorsingRightsDelegate {
    pub delegate: SignaturePublicKeyHash,
    pub first_slot: Slot,
    pub endorsing_power: EndorsingPower,
}

#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum EndorsingRightsError {
    #[error(transparent)]
    Response(#[from] ProtocolRpcResponseError),
    #[error("JSON parse error: {0}")]
    ToJson(String),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
}

impl From<serde_json::Error> for EndorsingRightsError {
    fn from(error: serde_json::Error) -> Self {
        Self::ToJson(error.to_string())
    }
}

pub(super) fn endorsing_rights_from_rpc_response(
    res: ProtocolRpcResponse,
) -> EndorsingRightsResponse {
    let body = res.ok_body_or().map_err(ProtocolRpcResponseError::from)?;

    #[derive(serde::Deserialize)]
    pub struct EndorsingRightsLevelJson {
        pub level: i32,
        pub delegates: Vec<EndorsingRightsDelegate>,
        pub estimated_time: Option<String>,
    }

    let rights = serde_json::from_str::<Vec<EndorsingRightsLevelJson>>(&body)?;

    let rights = rights
        .into_iter()
        .map(|erl| EndorsingRightsLevel {
            level: erl.level,
            delegates: erl.delegates,
        })
        .collect();

    Ok(rights)
}

pub type CycleDelegatesResponse = Result<CycleDelegates, CycleDelegatesError>;

pub type CycleDelegates = BTreeMap<SignaturePublicKeyHash, SignaturePublicKey>;

#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum CycleDelegatesError {
    #[error(transparent)]
    Response(#[from] ProtocolRpcResponseError),
    #[error("JSON parse error: {0}")]
    ToJson(String),
    #[error(transparent)]
    Conversion(#[from] ConversionError),
}

impl From<serde_json::Error> for CycleDelegatesError {
    fn from(error: serde_json::Error) -> Self {
        Self::ToJson(error.to_string())
    }
}

pub(super) fn cycle_delegates_from_rpc_response(
    res: ProtocolRpcResponse,
) -> CycleDelegatesResponse {
    let body = res.ok_body_or().map_err(ProtocolRpcResponseError::from)?;

    #[derive(serde::Deserialize)]
    struct ProtocolResultJson {
        support: SupportJson,
    }
    #[derive(serde::Deserialize)]
    struct SupportJson {
        elements: Vec<SignaturePublicKey>,
    }
    let json = serde_json::from_str::<ProtocolResultJson>(&body)?;
    let delegates = json
        .support
        .elements
        .into_iter()
        .map(|pk| pk.pk_hash().map(|pkh| (pkh, pk)))
        .collect::<Result<_, _>>()?;
    Ok(delegates)
}

pub type ValidatorsResponse = Result<Validators, ValidatorsError>;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Validators {
    pub validators: ValidatorsTable,
    pub slots: ValidatorSlots,
}

pub type ValidatorsTable = Vec<SignaturePublicKeyHash>;
pub type ValidatorSlots = BTreeMap<SignaturePublicKeyHash, Vec<Slot>>;

#[derive(Debug, Clone, thiserror::Error, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub enum ValidatorsError {
    #[error(transparent)]
    Response(#[from] ProtocolRpcResponseError),
    #[error("Missing delegate for slot `{0}`")]
    MissingDelegate(Slot),
    #[error("JSON parse error: {0}")]
    Json(String),
}

impl From<serde_json::Error> for ValidatorsError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

pub(super) fn validators_from_rpc_response(res: ProtocolRpcResponse) -> ValidatorsResponse {
    let body = res.ok_body_or().map_err(ProtocolRpcResponseError::from)?;

    #[derive(serde::Deserialize)]
    pub struct ValidatorJson {
        delegate: SignaturePublicKeyHash,
        slots: Vec<Slot>,
    }

    let rpc_validators = serde_json::from_str::<Vec<ValidatorJson>>(&body)?;
    let slots = rpc_validators
        .iter()
        .fold(Vec::new(), |mut slots, validator| {
            for slot in &validator.slots {
                let slot = *slot as usize;
                if slot >= slots.len() {
                    slots.resize(slot + 1, Option::<&SignaturePublicKeyHash>::None);
                }
                slots[slot] = Some(&validator.delegate);
            }
            slots
        });

    let validators = slots
        .into_iter()
        .enumerate()
        .map(|(i, d)| {
            d.cloned()
                .ok_or(ValidatorsError::MissingDelegate(i as Slot))
        })
        .collect::<Result<_, _>>()?;
    let slots = rpc_validators
        .into_iter()
        .map(|validator| (validator.delegate, validator.slots))
        .collect();

    Ok(Validators { validators, slots })
}
