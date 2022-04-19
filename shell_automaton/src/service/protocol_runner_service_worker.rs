// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::process::ExitStatus;
use std::sync::Arc;
use std::{ops::ControlFlow, os::unix::prelude::ExitStatusExt};

use crypto::hash::BlockHash;

use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};
use slog::Logger;
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_context_api::TezosContextStorageConfiguration;
use tezos_protocol_ipc_client::{ProtocolRunnerApi, ProtocolRunnerConnection, ProtocolRunnerError};
use tezos_protocol_ipc_messages::{InitProtocolContextParams, ProtocolMessage};
use tokio::process::Child;
use tokio::sync::Mutex;

use crate::protocol_runner::ProtocolRunnerToken;

use super::protocol_runner_service::{
    ProtocolRunnerRequest, ProtocolRunnerResponse, ProtocolRunnerResult,
};
use super::service_async_channel::{
    ServiceWorkerAsyncResponder, ServiceWorkerAsyncResponderSender,
};

pub type ProtocolRunnerResponder =
    ServiceWorkerAsyncResponder<ProtocolRunnerRequest, ProtocolRunnerResponse>;
pub type ProtocolRunnerResponderSender = ServiceWorkerAsyncResponderSender<ProtocolRunnerResponse>;

pub struct ProtocolRunnerServiceWorker {
    api: ProtocolRunnerApi,
    channel: ProtocolRunnerResponder,
    child_process_handle: Option<Child>,
    tezos_runtime_configuration: Option<TezosRuntimeConfiguration>,
    init_protocol_context_params: Option<InitProtocolContextParams>,
    init_protocol_context_ipc_cfg: Option<TezosContextStorageConfiguration>,
    log: Logger,

    // FIXME: doesn't properly handle `ForMempool` messages, but those are not used right now.
    /// ValidateOperation requests will all use a single connection, which means
    /// That only a single such request will be processed at a time.
    /// The predecessor hash will be checked before sending the request to the protocol
    /// runner. If it doesn't match, the request will be ignored.
    /// The predecessor hash changes when a BeginConstruction request is issued.
    /// This request creates a new connection and will not be blocked by pending
    /// ValidateOperation requests so that pending stale operations get ignored as soon
    /// as possible when the hash changes (on both the shell and protocol runner).
    prevalidator_predecessor_hash: Arc<Mutex<Option<BlockHash>>>,
    validate_operation_connection_alive: bool,
    validate_operation_connection: Arc<Mutex<Option<ProtocolRunnerConnection>>>,
}

impl ProtocolRunnerServiceWorker {
    pub fn new(api: ProtocolRunnerApi, channel: ProtocolRunnerResponder, log: Logger) -> Self {
        let child_process_handle = None;
        let tezos_runtime_configuration = None;
        let init_protocol_context_params = None;
        let init_protocol_context_ipc_cfg = None;
        let prevalidator_predecessor_hash = Arc::new(Mutex::new(None));
        let validate_operation_connection = Arc::new(Mutex::new(None));
        let validate_operation_connection_alive = false;

        Self {
            api,
            channel,
            child_process_handle,
            tezos_runtime_configuration,
            init_protocol_context_params,
            init_protocol_context_ipc_cfg,
            validate_operation_connection_alive,
            prevalidator_predecessor_hash,
            validate_operation_connection,
            log,
        }
    }

    /// Spawn the worker loop
    pub fn run(mut self) {
        self.api.tokio_runtime.clone().block_on(async move {
            while let ControlFlow::Continue(()) = self.process_next_message().await  {
                // continue
            }

            // Shut down the child process if we exit the loop and it is still up
            if let Some(mut child) = std::mem::take(&mut self.child_process_handle) {
                if let Err(err) = Self::terminate_or_kill(&mut child, "Protocol Runner Service loop ended".into()).await {
                    slog::warn!(self.log, "Failure when shutting down the protocol runner subprocess"; "error" => err);
                }
            }
        });
    }

    /// Process the next message.
    async fn process_next_message(&mut self) -> ControlFlow<()> {
        // Make sure that the protocol runner is still alive, if not, launch it.
        let result = if let Some(child) = &mut self.child_process_handle {
            tokio::select! {
                biased;

                exit_result = child.wait() => {
                    self.restart_protocol_runner(&exit_result).await;
                    self.validate_operation_connection_alive = false;
                    Err(exit_result)
                }

                req = self.channel.recv() => Ok(req)
            }
        } else {
            Ok(self.channel.recv().await)
        };

        let req = match result {
            Ok(Ok(req)) => req,
            Ok(Err(_)) => {
                // No more messages, exit the loop
                return ControlFlow::Break(());
            }
            Err(_) => {
                // Had to restart protocol runner, continue
                return ControlFlow::Continue(());
            }
        };

        let sender = self.channel.sender();
        let (token, req) = match req {
            ProtocolRunnerRequest::SpawnServer(()) => {
                let start_result = match self.api.start(None).await {
                    Ok(child) => {
                        self.child_process_handle = Some(child);
                        Ok(())
                    }
                    Err(err) => Err(err),
                };

                sender
                    .send(ProtocolRunnerResult::SpawnServer(start_result))
                    .await
                    .unwrap();
                return ControlFlow::Continue(());
            }
            ProtocolRunnerRequest::ShutdownServer(()) => {
                let result = if let Some(mut child) = self.child_process_handle.take() {
                    Self::terminate_or_kill(&mut child, "Shutdown requested".into()).await
                } else {
                    Ok(())
                };
                // TODO: maybe be explicit the protocol runner not being up when we try to shut it down?
                sender
                    .send(ProtocolRunnerResult::ShutdownServer(result))
                    .await
                    .unwrap();
                return ControlFlow::Continue(());
            }
            ProtocolRunnerRequest::Message(v) => {
                self.keep_protocol_runner_configuration(&v);
                v
            }
        };

        // Set this hash so that pending validation requests for the old predecessor
        // get dropped now that we changed the validator predecessor.
        if let ProtocolMessage::BeginConstructionForPrevalidationCall(req) = &req {
            let mut hash = self.prevalidator_predecessor_hash.lock().await;
            *hash = Some(req.predecessor_hash.clone());
        }

        // For ValidateOperation messages we use a single connection because there rate
        // of connections can get too high otherwise.
        if Self::is_validate_operation_message(&req) {
            if !self.validate_operation_connection_alive {
                if !self
                    .restore_validate_operation_connection(&req, token)
                    .await
                {
                    return ControlFlow::Continue(());
                }

                self.validate_operation_connection_alive = true;
            }

            tokio::spawn(Self::handle_validate_operation_message(
                sender,
                Arc::clone(&self.validate_operation_connection),
                Arc::clone(&self.prevalidator_predecessor_hash),
                (token, req),
                self.log.clone(),
            ));
        } else if let Some(conn) = self.make_connection(&req, token).await {
            tokio::spawn(Self::handle_protocol_message(
                sender,
                conn,
                (token, req),
                self.log.clone(),
            ));
        }

        ControlFlow::Continue(())
    }

    // Helper methods

    async fn handle_protocol_message(
        channel: ProtocolRunnerResponderSender,
        mut conn: ProtocolRunnerConnection,
        (token, req): (ProtocolRunnerToken, ProtocolMessage),
        log: Logger,
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
                // TODO: here, if the result is an error, we want to retry
                let _ = channel
                    .send(ProtocolRunnerResult::ApplyBlock((token, res)))
                    .await;
            }
            ProtocolMessage::BeginConstructionForPrevalidationCall(req)
            | ProtocolMessage::BeginConstructionForMempoolCall(req) => {
                let res = conn.begin_construction_for_prevalidation(req).await;
                let _ = channel
                    .send(ProtocolRunnerResult::BeginConstruction((token, res)))
                    .await;
            }
            ProtocolMessage::ContextGetLatestContextHashes(count) => {
                let res = conn.latest_context_hashes(count).await;
                let _ = channel
                    .send(ProtocolRunnerResult::GetCurrentHead((token, res)))
                    .await;
            }
            _other => {
                // TODO: say which message
                slog::warn!(
                    log,
                    "Received unexpected message in handle_protocol_message"
                );
            }
        }
    }

    async fn handle_validate_operation_message(
        channel: ProtocolRunnerResponderSender,
        conn_mutex: Arc<Mutex<Option<ProtocolRunnerConnection>>>,
        prevalidator_predecessor_hash: Arc<Mutex<Option<BlockHash>>>,
        (token, req): (ProtocolRunnerToken, ProtocolMessage),
        log: Logger,
    ) {
        let mut conn_lock = conn_mutex.lock().await;
        let conn = conn_lock.as_mut().unwrap();

        match req {
            ProtocolMessage::ValidateOperationForPrevalidationCall(req) => {
                {
                    let hash = prevalidator_predecessor_hash.lock().await;
                    if Some(&req.prevalidator.predecessor) != hash.as_ref() {
                        // Stale request, skip
                        return;
                    }
                }
                let res = conn.validate_operation_for_prevalidation(req).await;
                let _ = channel
                    .send(ProtocolRunnerResult::ValidateOperation((token, res)))
                    .await;
            }
            ProtocolMessage::ValidateOperationForMempoolCall(req) => {
                let res = conn.validate_operation_for_mempool(req).await;
                let _ = channel
                    .send(ProtocolRunnerResult::ValidateOperation((token, res)))
                    .await;
            }
            _other => {
                // TODO: say which message
                slog::warn!(
                    log,
                    "Received unexpected message in handle_validate_operation_message"
                );
            }
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

    fn is_validate_operation_message(req: &ProtocolMessage) -> bool {
        matches!(
            req,
            ProtocolMessage::ValidateOperationForPrevalidationCall(_)
                | ProtocolMessage::ValidateOperationForMempoolCall(_)
        )
    }

    /// Attempt to restart the protocol runner
    async fn restart_protocol_runner(&mut self, exit_result: &Result<ExitStatus, std::io::Error>) {
        let exit_status = exit_result.as_ref().unwrap();

        // We assume we can always restart here (and the node is not shutting down)
        // because otherwise `child_process_handle` would be `None` because of
        // how the `ShutdownServer` handle takes the value out of it.
        if let Some(code) = exit_status.code() {
            slog::warn!(self.log, "Child process exited"; "code" => code);
        } else {
            slog::warn!(self.log, "Child process was terminated by signal"; "signal" => exit_status.signal());
        }

        self.child_process_handle.take();

        slog::info!(self.log, "Restarting child process...");

        match self.api.start(None).await {
            Ok(child) => {
                slog::info!(self.log, "Child process restarted sucessfully");
                self.child_process_handle.replace(child);

                // TODO: remove these unwraps, handle such failures more gracefully

                let mut conn = self.api.connect().await.unwrap();

                if let Some(config) = &self.tezos_runtime_configuration {
                    slog::info!(self.log, "Restoring runtime configuration...");
                    conn.change_runtime_configuration(config.clone())
                        .await
                        .unwrap();
                }

                if let Some(params) = &self.init_protocol_context_params {
                    slog::info!(self.log, "Re-initializing context...");
                    conn.init_protocol_context_raw(params.clone())
                        .await
                        .unwrap();
                }

                if let Some(cfg) = &self.init_protocol_context_ipc_cfg {
                    slog::info!(self.log, "Re-initializing context IPC server");
                    conn.init_context_ipc_server_raw(cfg.clone()).await.unwrap();
                }
            }
            Err(err) => {
                slog::warn!(self.log, "Attempt to restart child process failed"; "reason" => err)
            }
        };
    }

    fn keep_protocol_runner_configuration(&mut self, v: &(ProtocolRunnerToken, ProtocolMessage)) {
        match &v.1 {
            // Keep these around to be able to properly reinitialize the protocol
            // runner after a restart
            ProtocolMessage::ChangeRuntimeConfigurationCall(cfg) => {
                self.tezos_runtime_configuration = Some(cfg.clone());
            }
            ProtocolMessage::InitProtocolContextCall(params) => {
                self.init_protocol_context_params = Some(params.clone());
            }
            ProtocolMessage::InitProtocolContextIpcServer(cfg) => {
                self.init_protocol_context_ipc_cfg = Some(cfg.clone());
            }
            _ => (),
        }
    }

    /// Make a new connection to the protocol runner
    async fn make_connection(
        &self,
        req: &ProtocolMessage,
        token: ProtocolRunnerToken,
    ) -> Option<ProtocolRunnerConnection> {
        match self.api.connect().await {
            Ok(v) => Some(v),
            Err(err) => {
                let _ = self
                    .channel
                    .send(match *req {
                        ProtocolMessage::ChangeRuntimeConfigurationCall(_) => {
                            ProtocolRunnerResult::InitRuntime((token, Err(err.into())))
                        }
                        ProtocolMessage::InitProtocolContextCall(_) => {
                            ProtocolRunnerResult::InitContext((token, Err(err.into())))
                        }
                        ProtocolMessage::InitProtocolContextIpcServer(_) => {
                            ProtocolRunnerResult::InitContextIpcServer((token, Err(err.into())))
                        }
                        ProtocolMessage::ApplyBlockCall(_) => {
                            ProtocolRunnerResult::ApplyBlock((token, Err(err.into())))
                        }
                        ProtocolMessage::BeginConstructionForPrevalidationCall(_)
                        | ProtocolMessage::BeginConstructionForMempoolCall(_) => {
                            ProtocolRunnerResult::BeginConstruction((token, Err(err.into())))
                        }
                        _ => panic!("Unexpected protocol runner message"),
                    })
                    .await
                    .ok();
                None
            }
        }
    }

    /// Try to restore the connection used for operation validation requests.
    /// Returns true if the connection was restored successfuly.
    async fn restore_validate_operation_connection(
        &self,
        req: &ProtocolMessage,
        token: ProtocolRunnerToken,
    ) -> bool {
        let mut conn_lock = Arc::clone(&self.validate_operation_connection)
            .lock_owned()
            .await;
        if conn_lock.is_none() {
            *conn_lock = match self.api.connect().await {
                Ok(v) => Some(v),
                Err(err) => {
                    let _ = self
                        .channel
                        .send(match *req {
                            ProtocolMessage::ValidateOperationForPrevalidationCall(_)
                            | ProtocolMessage::ValidateOperationForMempoolCall(_) => {
                                ProtocolRunnerResult::ValidateOperation((token, Err(err.into())))
                            }
                            _ => panic!("Unexpected prevalidator request"),
                        })
                        .await
                        .ok();
                    None
                }
            }
        }

        conn_lock.is_some()
    }
}
