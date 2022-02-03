// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use serde::{Deserialize, Serialize};

use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::{
    ApplyBlockRequest, ApplyBlockResponse, CommitGenesisResult, InitProtocolContextResult,
    TezosRuntimeConfiguration,
};
use tezos_context_api::{PatchContext, TezosContextStorageConfiguration};
use tezos_protocol_ipc_client::{
    ProtocolRunnerApi, ProtocolRunnerConnection, ProtocolRunnerError, ProtocolServiceError,
};
use tezos_protocol_ipc_messages::{
    GenesisResultDataParams, InitProtocolContextParams, ProtocolMessage,
};
use tokio::process::Child;

use crate::protocol_runner::ProtocolRunnerToken;

use super::service_async_channel::{
    worker_channel, ResponseTryRecvError, ServiceWorkerAsyncRequester, ServiceWorkerAsyncResponder,
    ServiceWorkerAsyncResponderSender,
};

pub type ProtocolRunnerResponse = ProtocolRunnerResult;

#[derive(Debug)]
pub enum ProtocolRunnerRequest {
    SpawnServer(()),
    ShutdownServer(()),
    Message((ProtocolRunnerToken, ProtocolMessage)),
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
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

    ApplyBlock(
        (
            ProtocolRunnerToken,
            Result<ApplyBlockResponse, ProtocolServiceError>,
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
            Self::ApplyBlock((token, _)) => Some(*token),
            Self::ShutdownServer(_) => None,
        }
    }
}

pub type ProtocolRunnerRequester =
    ServiceWorkerAsyncRequester<ProtocolRunnerRequest, ProtocolRunnerResponse>;
pub type ProtocolRunnerResponder =
    ServiceWorkerAsyncResponder<ProtocolRunnerRequest, ProtocolRunnerResponse>;
pub type ProtocolRunnerResponderSender = ServiceWorkerAsyncResponderSender<ProtocolRunnerResponse>;

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
    async fn handle_protocol_message(
        channel: ProtocolRunnerResponderSender,
        mut conn: ProtocolRunnerConnection,
        (token, req): (ProtocolRunnerToken, ProtocolMessage),
    ) {
        match req {
            ProtocolMessage::ChangeRuntimeConfigurationCall(config) => {
                let res = conn.change_runtime_configuration(config).await;
                let _ = channel
                    .send(ProtocolRunnerResult::InitRuntime((token, res)))
                    .await;
            }
            ProtocolMessage::InitProtocolContextCall(params) => {
                let res = conn.init_protocol_context_raw(params).await;
                let _ = channel
                    .send(ProtocolRunnerResult::InitContext((token, res)))
                    .await;
            }
            ProtocolMessage::InitProtocolContextIpcServer(cfg) => {
                let res = conn.init_context_ipc_server_raw(cfg).await;
                let _ = channel
                    .send(ProtocolRunnerResult::InitContextIpcServer((token, res)))
                    .await;
            }
            ProtocolMessage::GenesisResultDataCall(params) => {
                let res = conn.genesis_result_data_raw(params).await;
                let _ = channel
                    .send(ProtocolRunnerResult::GenesisCommitResultGet((token, res)))
                    .await;
            }
            ProtocolMessage::ApplyBlockCall(req) => {
                let res = conn.apply_block(req).await;
                let _ = channel
                    .send(ProtocolRunnerResult::ApplyBlock((token, res)))
                    .await;
            }
            _ => unimplemented!(),
        }
    }

    /// Will try to shutdown the children process, first with a SIGINT, and then with a SIGKILL
    async fn terminate_or_kill(
        process: &mut Child,
        reason: String,
    ) -> Result<(), ProtocolRunnerError> {
        // try to send SIGINT (ctrl-c)
        if let Some(pid) = process.id() {
            let pid = Pid::from_raw(pid as i32);
            match signal::kill(pid, Signal::SIGINT) {
                Ok(_) => Ok(()),
                Err(sigint_error) => {
                    // (fallback) if SIGINT failed, we just kill process
                    match process.kill().await {
                        Ok(_) => Ok(()),
                        Err(kill_error) => Err(ProtocolRunnerError::TerminateError {
                            reason: format!(
                                "Reason for termination: {}, sigint_error: {}, kill_error: {}",
                                reason, sigint_error, kill_error
                            ),
                        }),
                    }
                }
            }
        } else {
            Ok(())
        }
    }

    fn run(mut channel: ProtocolRunnerResponder, mut api: ProtocolRunnerApi) {
        api.tokio_runtime.clone().block_on(async move {
            let mut child_process_handle = None;

            while let Ok(req) = channel.recv().await {
                let sender = channel.sender();

                let (token, req) = match req {
                    ProtocolRunnerRequest::SpawnServer(()) => {
                        let start_result = match api.start(None).await {
                            Ok(child) => {
                                child_process_handle = Some(child);
                                Ok(())
                            }
                            Err(err) => Err(err),
                        };

                        sender
                            .send(ProtocolRunnerResult::SpawnServer(start_result))
                            .await
                            .unwrap();
                        continue;
                    }
                    ProtocolRunnerRequest::ShutdownServer(()) => {
                        let result = if let Some(mut child) = child_process_handle.take() {
                            Self::terminate_or_kill(&mut child, "Shutdown requested".into()).await
                        } else {
                            Ok(())
                        };
                        // TODO: maybe be explicit the protocol runner not being up when we try to shut it down?
                        sender
                            .send(ProtocolRunnerResult::ShutdownServer(result))
                            .await
                            .unwrap();
                        continue;
                    }
                    ProtocolRunnerRequest::Message(v) => v,
                };

                let conn = match api.connect().await {
                    Ok(v) => v,
                    Err(err) => {
                        let _ = channel
                            .send(match req {
                                ProtocolMessage::ChangeRuntimeConfigurationCall(_) => {
                                    ProtocolRunnerResult::InitRuntime((token, Err(err.into())))
                                }
                                ProtocolMessage::InitProtocolContextCall(_) => {
                                    ProtocolRunnerResult::InitContext((token, Err(err.into())))
                                }
                                ProtocolMessage::InitProtocolContextIpcServer(_) => {
                                    ProtocolRunnerResult::InitContextIpcServer((
                                        token,
                                        Err(err.into()),
                                    ))
                                }
                                ProtocolMessage::ApplyBlockCall(_) => {
                                    ProtocolRunnerResult::ApplyBlock((token, Err(err.into())))
                                }
                                _ => unimplemented!(),
                            })
                            .await;
                        continue;
                    }
                };

                tokio::spawn(Self::handle_protocol_message(sender, conn, (token, req)));
            }

            // Shut down the child process if we exit the loop and it is still up
            if let Some(mut child) = std::mem::take(&mut child_process_handle) {
                // TODO: if this fails, it should be logged somewhere
                Self::terminate_or_kill(&mut child, "Protocol Runner Service loop ended".into())
                    .await
                    .ok();
            }
        });
    }

    pub fn new(
        api: ProtocolRunnerApi,
        mio_waker: Arc<mio::Waker>,
        bound: usize,
        status_sender: tokio::sync::watch::Sender<bool>,
    ) -> Self {
        let (c1, c2) = worker_channel(mio_waker, bound);
        thread::spawn(|| Self::run(c2, api));

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
            turn_off_context_raw_inspector: true, // TODO - TE-261: remove later, new context doesn't use it
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

    fn notify_status(&mut self, initialized: bool) {
        let _ = self.status_sender.send(initialized);
    }

    fn shutdown(&mut self) {
        self.channel
            .blocking_send(ProtocolRunnerRequest::ShutdownServer(()))
            .unwrap();
    }
}
