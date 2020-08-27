// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This crate provides core implementation for a protocol runner (both IPC server and client parts).

use std::time::Duration;

use r2d2::{CustomizeConnection, Pool};
use slog::Logger;

use crate::pool::{InitReadonlyContextProtocolRunnerConnectionCustomizer, NoopProtocolRunnerConnectionCustomizer, PoolError, ProtocolRunnerConnection, ProtocolRunnerManager};
use crate::service::{ExecutableProtocolRunner, ProtocolEndpointConfiguration};

mod pool;
pub mod protocol;
pub mod service;

/// Configuration for tezos api pool
#[derive(Debug, Clone)]
pub struct TezosApiConnectionPoolConfiguration {
    /// Nuber of connections to create on startup of pool (can be 0, so the connections are created on demand)
    pub min_connections: u8,
    pub max_connections: u8,
    /// wait for connection to protocol_runner
    pub connection_timeout: Duration,

    /// 'max_lifetime' for one protocol_runner
    pub max_lifetime: Duration,
    /// if protocol_runner is not used 'idle_timeout', than is closed
    pub idle_timeout: Duration,
}

/// This pool is "hard-coded" for ExecutableProtocolRunner, but it is easily extended as [TezosApiConnectionPool<Runner: ProtocolRunner + 'static>] if needed
pub type RunnerType = ExecutableProtocolRunner;

/// Wrapper for r2d2 pool with managed protocol_runner "connections", protocol runners sub-processes are now managed and started by the pool.
/// Automatically refreshes old protocol_runner sub-processes [idle_timeout][max_lifetime]
///
/// One connection means one protocol_runner sub-process and one IPC
pub struct TezosApiConnectionPool {
    pub pool: Pool<ProtocolRunnerManager<RunnerType>>,
    pub pool_name: String,
}

impl TezosApiConnectionPool {
    /// Pool with ffi initialized context for readonly - see description AT_LEAST_ONE_WRITE_PROTOCOL_CONTEXT_WAS_SUCCESS_AT_FIRST_LOCK
    pub fn new_with_readonly_context(
        pool_name: String,
        pool_cfg: TezosApiConnectionPoolConfiguration,
        endpoint_cfg: ProtocolEndpointConfiguration,
        log: Logger) -> TezosApiConnectionPool {
        Self::new(
            pool_name,
            pool_cfg,
            endpoint_cfg,
            log,
            Box::new(InitReadonlyContextProtocolRunnerConnectionCustomizer),
        )
    }

    /// Pool without ffi initialized context
    /// Can be used for operation like decode_context_value, which does not access context
    pub fn new_without_context(
        pool_name: String,
        pool_cfg: TezosApiConnectionPoolConfiguration,
        endpoint_cfg: ProtocolEndpointConfiguration,
        log: Logger) -> TezosApiConnectionPool {
        Self::new(
            pool_name,
            pool_cfg,
            endpoint_cfg,
            log,
            Box::new(NoopProtocolRunnerConnectionCustomizer),
        )
    }

    fn new(
        pool_name: String,
        pool_cfg: TezosApiConnectionPoolConfiguration,
        endpoint_cfg: ProtocolEndpointConfiguration,
        log: Logger,
        initializer: Box<dyn CustomizeConnection<ProtocolRunnerConnection<RunnerType>, PoolError>>) -> TezosApiConnectionPool {

        // create manager
        let manager = ProtocolRunnerManager::<RunnerType>::new(pool_name.clone(), endpoint_cfg, log);

        // create pool for ffi protocol runner connections
        let pool = r2d2::Pool::builder()
            .min_idle(Some(pool_cfg.min_connections as u32))
            .max_size(pool_cfg.max_connections as u32)
            .connection_timeout(pool_cfg.connection_timeout)
            .max_lifetime(Some(pool_cfg.max_lifetime))
            .idle_timeout(Some(pool_cfg.idle_timeout))
            .connection_customizer(initializer)
            .build(manager)
            .unwrap();

        TezosApiConnectionPool {
            pool,
            pool_name,
        }
    }
}

impl Drop for TezosApiConnectionPool {
    fn drop(&mut self) {
        // TODO: ensure all connections are dropped and protocol_runners are closed
    }
}