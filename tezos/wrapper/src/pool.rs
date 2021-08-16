// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cell::Cell;
use std::fmt::Formatter;
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use std::{error, fmt};

use r2d2::{CustomizeConnection, HandleError, ManageConnection};
use slog::{debug, error, info, o, warn, Logger};

use ipc::IpcError;

use crate::runner::{ExecutableProtocolRunner, ProtocolRunnerError};
use crate::service::{ProtocolController, ProtocolRunnerEndpoint, ProtocolServiceError};
use crate::ProtocolEndpointConfiguration;

/// Possible errors for storage
#[derive(Debug)]
pub enum PoolError {
    SpawnRunnerError {
        error: ProtocolRunnerError,
    },
    IpcError {
        reason: String,
        error: IpcError,
    },
    InitContextError {
        message: String,
        error: ProtocolServiceError,
    },
}

impl std::error::Error for PoolError {}

impl fmt::Display for PoolError {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match *self {
            PoolError::SpawnRunnerError { ref error } => write!(f, "Create pool connection error - fail to spawn sub-process, reason: {:?}", error),
            PoolError::IpcError { ref reason, ref error } => write!(f, "Create pool connection IPC error - {}, error: {:?}", reason, error),
            PoolError::InitContextError { ref message, ref error } => write!(f, "Create pool connection error - fail to initialize context, message: {}, reason: {:?}", message, error),
        }
    }
}

/// Will terminate the subprocess in an asynchronous manner without blocking
/// Returns immediately, doesn't mean that the process has already stopped.
fn terminate_subprocess_without_blocking(
    tokio_runtime: &tokio::runtime::Handle,
    subprocess: tokio::process::Child,
    log: Logger,
) {
    let _enter = tokio_runtime.enter();
    tokio::task::spawn(terminate_subprocess_task(subprocess, log));
}

fn terminate_subprocess_task(
    mut subprocess: tokio::process::Child,
    log: Logger,
) -> impl Future<Output = ()> {
    async move {
        if let Err(e) = ExecutableProtocolRunner::wait_and_terminate_ref(
            &mut subprocess,
            ExecutableProtocolRunner::PROCESS_TERMINATE_WAIT_TIMEOUT,
            &log,
        )
        .await
        {
            warn!(log, "Failed to terminate/kill protocol runner"; "reason" => e);
        }
    }
}

/// Protocol runner sub-process wrapper which acts as connection
pub struct ProtocolRunnerConnection {
    pub api: ProtocolController,
    subprocess: Option<tokio::process::Child>,
    log: Logger,
    pub name: String,
    tokio_runtime: tokio::runtime::Handle,

    /// Indicates that we want to release this connection on return to pool (used for gracefull shutdown)
    release_on_return_to_pool: Cell<bool>,
}

impl ProtocolRunnerConnection {
    pub fn is_valid(&mut self) -> Result<(), PoolError> {
        // TODO: check if unix socket is connected?
        Ok(())
    }

    fn has_broken(&mut self) -> bool {
        if self.release_on_return_to_pool.get() {
            return true;
        }
        let is_subprocess_running = if let Some(subprocess) = &mut self.subprocess {
            ExecutableProtocolRunner::is_running(subprocess)
        } else {
            false
        };
        !is_subprocess_running
    }

    pub fn terminate_subprocess(&mut self) {
        // try shutdown gracefully
        if let Err(e) = self.api.shutdown() {
            warn!(self.log, "Failed to shutdown protocol runner gracefully"; "reason" => e);
        };

        // try terminate sub-process (if running)
        if let Some(subprocess) = std::mem::take(&mut self.subprocess) {
            terminate_subprocess_without_blocking(
                &self.tokio_runtime,
                subprocess,
                self.log.clone(),
            );
        }
    }

    /// Mark connection as "destroy" when return back to pool
    pub fn set_release_on_return_to_pool(&self) {
        self.release_on_return_to_pool.set(true);
    }

    /// Logs exit status of child process
    pub fn log_exit_status(&mut self) {
        if let Some(subprocess) = &mut self.subprocess {
            ExecutableProtocolRunner::log_exit_status(subprocess, &self.log);
        }
    }
}

/// Connection manager, which creates new connections:
/// - runs new sub-process
/// - starts IPC accept
pub struct ProtocolRunnerManager {
    pool_name: String,
    pool_name_counter: AtomicUsize,
    pool_connection_timeout: Duration,

    tokio_runtime: tokio::runtime::Handle,

    pub endpoint_cfg: ProtocolEndpointConfiguration,
    pub log: Logger,
}

impl ProtocolRunnerManager {
    const MIN_ACCEPT_TIMEOUT: Duration = Duration::from_secs(3);

    pub fn new(
        pool_name: String,
        pool_connection_timeout: Duration,
        endpoint_cfg: ProtocolEndpointConfiguration,
        tokio_runtime: tokio::runtime::Handle,
        log: Logger,
    ) -> Self {
        Self {
            pool_name,
            pool_name_counter: AtomicUsize::new(1),
            pool_connection_timeout,
            endpoint_cfg,
            tokio_runtime,
            log,
        }
    }

    pub fn create_connection(&self) -> Result<ProtocolRunnerConnection, PoolError> {
        let endpoint_name = format!(
            "{}_{:?}",
            &self.pool_name,
            self.pool_name_counter.fetch_add(1, Ordering::SeqCst)
        );

        // crate protocol runner endpoint
        let protocol_endpoint = ProtocolRunnerEndpoint::try_new(
            &endpoint_name,
            self.endpoint_cfg.clone(),
            self.tokio_runtime.clone(),
            self.log.new(o!("endpoint" => endpoint_name.clone())),
        )
        .map_err(|error| PoolError::IpcError {
            reason: "fail to setup protocol runner endpoint".to_string(),
            error,
        })?;

        // start sub-process
        let (subprocess, mut protocol_commands) = match protocol_endpoint.start() {
            Ok(subprocess) => {
                let ProtocolRunnerEndpoint { commands, .. } = protocol_endpoint;
                (subprocess, commands)
            }
            Err(e) => {
                error!(self.log, "Failed to spawn protocol runner process"; "endpoint" => endpoint_name, "reason" => &e);
                return Err(PoolError::SpawnRunnerError { error: e });
            }
        };

        // count accept_timeout according to connection_timeout
        let accept_timeout = match self.pool_connection_timeout.checked_div(3) {
            Some(conn_timeout) => std::cmp::max(Self::MIN_ACCEPT_TIMEOUT, conn_timeout),
            None => Self::MIN_ACCEPT_TIMEOUT,
        };

        // start IPC connection
        let api = match protocol_commands.try_accept(accept_timeout) {
            Ok(controller) => controller,
            Err(e) => {
                error!(self.log, "Failed to accept IPC connection on sub-process (so terminate sub-process)"; "endpoint" => endpoint_name, "reason" => format!("{:?}", &e));
                // try terminate sub-process (if running)
                terminate_subprocess_without_blocking(
                    &self.tokio_runtime,
                    subprocess,
                    self.log.clone(),
                );
                return Err(PoolError::IpcError {
                    reason: "fail to accept IPC for sub-process".to_string(),
                    error: e,
                });
            }
        };

        debug!(self.log, "Connection for protocol runner was created successfully"; "endpoint" => endpoint_name.clone());
        Ok(ProtocolRunnerConnection {
            api,
            subprocess: Some(subprocess),
            log: self.log.new(o!("endpoint" => endpoint_name.clone())),
            name: endpoint_name,
            tokio_runtime: self.tokio_runtime.clone(),
            release_on_return_to_pool: Cell::new(false),
        })
    }
}

impl ManageConnection for ProtocolRunnerManager {
    type Connection = ProtocolRunnerConnection;
    type Error = PoolError;

    fn connect(&self) -> Result<ProtocolRunnerConnection, PoolError> {
        self.create_connection()
    }

    fn is_valid(&self, conn: &mut ProtocolRunnerConnection) -> Result<(), PoolError> {
        conn.is_valid()
    }

    fn has_broken(&self, conn: &mut ProtocolRunnerConnection) -> bool {
        let has_broken = conn.has_broken();

        if has_broken {
            conn.log_exit_status();
        }

        has_broken
    }
}

/// Custom connection initializer, which tries to initialize readonly context
#[derive(Debug)]
pub struct InitReadonlyContextProtocolRunnerConnectionCustomizer;

impl CustomizeConnection<ProtocolRunnerConnection, PoolError>
    for InitReadonlyContextProtocolRunnerConnectionCustomizer
{
    fn on_acquire(&self, conn: &mut ProtocolRunnerConnection) -> Result<(), PoolError> {
        match conn.api.init_protocol_for_read() {
            Ok(_) => {
                info!(conn.log, "Connection for protocol runner was successfully initialized with readonly context");
                Ok(())
            }
            Err(e) => {
                error!(conn.log, "Failed to initialize readonly context (so terminate sub-process)"; "reason" => format!("{:?}", &e));
                conn.terminate_subprocess();
                Err(PoolError::InitContextError {
                    message: String::from("readonly_context"),
                    error: e,
                })
            }
        }
    }

    fn on_release(&self, mut conn: ProtocolRunnerConnection) {
        debug!(conn.log, "Closing connection for protocol runner");
        conn.terminate_subprocess();
        info!(
            conn.log,
            "Connection for protocol runner was successfully released"
        );
    }
}

/// Custom connection customizer, which just kills protocol_runner sub-process
#[derive(Debug)]
pub struct NoopProtocolRunnerConnectionCustomizer;

impl CustomizeConnection<ProtocolRunnerConnection, PoolError>
    for NoopProtocolRunnerConnectionCustomizer
{
    fn on_acquire(&self, conn: &mut ProtocolRunnerConnection) -> Result<(), PoolError> {
        info!(
            conn.log,
            "Connection for protocol runner was successfully initialized"
        );
        Ok(())
    }

    fn on_release(&self, mut conn: ProtocolRunnerConnection) {
        debug!(conn.log, "Closing connection for protocol runner");
        conn.terminate_subprocess();
        info!(
            conn.log,
            "Connection for protocol runner was successfully released"
        );
    }
}

/// Custom error logging handler, which logs errors with slog logger
#[derive(Clone, Debug)]
pub struct SlogErrorHandler(Logger, String);

impl SlogErrorHandler {
    pub fn new(logger: Logger, pool_name: String) -> Self {
        Self(logger, pool_name)
    }
}

impl<E> HandleError<E> for SlogErrorHandler
where
    E: error::Error,
{
    fn handle_error(&self, error: E) {
        error!(self.0, "Connection pool error";
                       "error" => format!("{}", error),
                       "pool_name" => self.1.clone());
    }
}
