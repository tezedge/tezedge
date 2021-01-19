// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
// #![forbid(unsafe_code)]

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use riker::actors::*;
use rocksdb::Cache;
use slog::{crit, debug, error, info, o, warn, Drain, Logger};

use logging::detailed_json;
use logging::file::FileAppenderBuilder;
use monitoring::{Monitor, WebsocketHandler};
use networking::p2p::network_channel::NetworkChannel;
use networking::ShellCompatibilityVersion;
use rpc::rpc_actor::RpcServer;
use shell::chain_feeder::ChainFeeder;
use shell::chain_manager::ChainManager;
use shell::context_listener::ContextListener;
use shell::mempool::init_mempool_state_storage;
use shell::mempool::mempool_prevalidator::MempoolPrevalidator;
use shell::peer_manager::PeerManager;
use shell::shell_channel::{ShellChannel, ShellChannelTopic, ShuttingDown};
use storage::context::TezedgeContext;
use storage::merkle_storage::MerkleStorage;
use storage::persistent::sequence::Sequences;
use storage::persistent::{open_cl, open_kv, CommitLogSchema, KeyValueSchema, PersistentStorage};
use storage::{
    block_storage, check_database_compatibility, context_action_storage,
    resolve_storage_init_chain_data, BlockMetaStorage, BlockStorage, ChainMetaStorage,
    ContextActionStorage, MempoolStorage, OperationsMetaStorage, OperationsStorage,
    PredecessorStorage, StorageInitInfo, SystemStorage,
};
use tezos_api::environment;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_identity::Identity;
use tezos_wrapper::runner::{ExecutableProtocolRunner, ProtocolRunner};
use tezos_wrapper::service::ProtocolRunnerEndpoint;
use tezos_wrapper::ProtocolEndpointConfiguration;
use tezos_wrapper::{TezosApiConnectionPool, TezosApiConnectionPoolConfiguration};

use crate::configuration::LogFormat;

mod configuration;
mod identity;
mod system;

const DATABASE_VERSION: i64 = 16;

macro_rules! shutdown_and_exit {
    ($err:expr, $sys:ident) => {{
        $err;
        futures::executor::block_on($sys.shutdown()).unwrap();
        return;
    }};
}

macro_rules! create_terminal_logger {
    ($type:expr) => {{
        match $type {
            LogFormat::Simple => slog_async::Async::new(
                slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
                    .build()
                    .fuse(),
            )
            .chan_size(32768)
            .overflow_strategy(slog_async::OverflowStrategy::Block)
            .build(),
            LogFormat::Json => {
                slog_async::Async::new(detailed_json::default(std::io::stdout()).fuse())
                    .chan_size(32768)
                    .overflow_strategy(slog_async::OverflowStrategy::Block)
                    .build()
            }
        }
    }};
}

macro_rules! create_file_logger {
    ($type:expr, $path:expr) => {{
        let appender = FileAppenderBuilder::new($path)
            .rotate_size(10_485_760 * 10) // 100 MB
            .rotate_keep(2)
            .rotate_compress(true)
            .build();

        match $type {
            LogFormat::Simple => slog_async::Async::new(
                slog_term::FullFormat::new(slog_term::PlainDecorator::new(appender))
                    .build()
                    .fuse(),
            )
            .chan_size(32768)
            .overflow_strategy(slog_async::OverflowStrategy::Block)
            .build(),
            LogFormat::Json => slog_async::Async::new(detailed_json::default(appender).fuse())
                .chan_size(32768)
                .overflow_strategy(slog_async::OverflowStrategy::Block)
                .build(),
        }
    }};
}

fn create_logger(env: &crate::configuration::Environment) -> Logger {
    let drain = match &env.logging.file {
        Some(log_file) => create_file_logger!(env.logging.format, log_file),
        None => create_terminal_logger!(env.logging.format),
    }
    .filter_level(env.logging.level)
    .fuse();

    Logger::root(drain, slog::o!())
}

fn create_tokio_runtime(env: &crate::configuration::Environment) -> tokio::runtime::Runtime {
    // use threaded work staling scheduler
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    // set number of threads in a thread pool
    if env.tokio_threads > 0 {
        builder.worker_threads(env.tokio_threads);
    }
    // build runtime
    builder.build().expect("Failed to create tokio runtime")
}

/// Create pool for ffi protocol runner connections (used just for readonly context)
/// Connections are created on demand, but depends on [TezosApiConnectionPoolConfiguration][min_connections]
fn create_tezos_readonly_api_pool(
    pool_name: &str,
    pool_cfg: TezosApiConnectionPoolConfiguration,
    env: &crate::configuration::Environment,
    tezos_env: TezosEnvironmentConfiguration,
    log: Logger,
) -> TezosApiConnectionPool {
    TezosApiConnectionPool::new_with_readonly_context(
        String::from(pool_name),
        pool_cfg,
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: env.logging.ocaml_log_enabled,
                no_of_ffi_calls_treshold_for_gc: env.ffi.no_of_ffi_calls_threshold_for_gc,
                debug_mode: false,
            },
            tezos_env,
            env.enable_testchain,
            &env.storage.tezos_data_dir,
            &env.ffi.protocol_runner,
            env.logging.level,
            false,
        ),
        log,
    )
}

/// Create pool for ffi protocol runner connections (used just for ffi calls which does not need context)
/// Connections are created on demand, but depends on [TezosApiConnectionPoolConfiguration][min_connections]
fn create_tezos_without_context_api_pool(
    pool_name: &str,
    pool_cfg: TezosApiConnectionPoolConfiguration,
    env: &crate::configuration::Environment,
    tezos_env: TezosEnvironmentConfiguration,
    log: Logger,
) -> TezosApiConnectionPool {
    TezosApiConnectionPool::new_without_context(
        String::from(pool_name),
        pool_cfg,
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: env.logging.ocaml_log_enabled,
                no_of_ffi_calls_treshold_for_gc: env.ffi.no_of_ffi_calls_threshold_for_gc,
                debug_mode: false,
            },
            tezos_env,
            env.enable_testchain,
            &env.storage.tezos_data_dir,
            &env.ffi.protocol_runner,
            env.logging.level,
            false,
        ),
        log,
    )
}

/// Create pool for ffi protocol runner connection (used for write to context)
/// There is limitation, that only one write connection to context can be open, so we limit this pool to 1.
/// This one connection is created at startup of the pool (min_connections=1).
#[allow(dead_code)]
fn create_tezos_writeable_api_pool(
    env: &crate::configuration::Environment,
    tezos_env: TezosEnvironmentConfiguration,
    log: Logger,
) -> TezosApiConnectionPool {
    TezosApiConnectionPool::new_without_context(
        String::from("tezos_write_api_pool"),
        TezosApiConnectionPoolConfiguration {
            idle_timeout: Duration::from_secs(1800),
            max_lifetime: Duration::from_secs(21600),
            connection_timeout: Duration::from_secs(60),
            min_connections: 1,
            max_connections: 1,
        },
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: env.logging.ocaml_log_enabled,
                no_of_ffi_calls_treshold_for_gc: env.ffi.no_of_ffi_calls_threshold_for_gc,
                debug_mode: env.storage.store_context_actions,
            },
            tezos_env,
            env.enable_testchain,
            &env.storage.tezos_data_dir,
            &env.ffi.protocol_runner,
            env.logging.level,
            true,
        ),
        log,
    )
}

fn block_on_actors(
    env: crate::configuration::Environment,
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: StorageInitInfo,
    identity: Arc<Identity>,
    actor_system: ActorSystem,
    persistent_storage: PersistentStorage,
    tezedge_context: TezedgeContext,
    log: Logger,
) {
    // if feeding is started, than run chain manager
    let is_sandbox = env.tezos_network == environment::TezosEnvironment::Sandbox;
    // version
    let shell_compatibility_version = Arc::new(ShellCompatibilityVersion::new(
        tezos_env.version.clone(),
        shell::SUPPORTED_DISTRIBUTED_DB_VERSION.to_vec(),
        shell::SUPPORTED_P2P_VERSION.to_vec(),
    ));

    // create pool for ffi protocol runner connections (used just for readonly context)
    let tezos_readonly_api_pool = Arc::new(create_tezos_readonly_api_pool(
        "tezos_readonly_api_pool",
        env.ffi.tezos_readonly_api_pool.clone(),
        &env,
        tezos_env.clone(),
        log.clone(),
    ));
    let tezos_readonly_prevalidation_api_pool = Arc::new(create_tezos_readonly_api_pool(
        "tezos_readonly_prevalidation_api",
        env.ffi.tezos_readonly_prevalidation_api_pool.clone(),
        &env,
        tezos_env.clone(),
        log.clone(),
    ));
    let tezos_without_context_api_pool = Arc::new(create_tezos_without_context_api_pool(
        "tezos_without_context_api_pool",
        env.ffi.tezos_without_context_api_pool.clone(),
        &env,
        tezos_env.clone(),
        log.clone(),
    ));

    // tezos protocol runner endpoint for applying blocks to chain
    let mut apply_blocks_protocol_runner_endpoint = match ProtocolRunnerEndpoint::<
        ExecutableProtocolRunner,
    >::try_new(
        "apply_blocks_protocol_runner_endpoint",
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: env.logging.ocaml_log_enabled,
                no_of_ffi_calls_treshold_for_gc: env.ffi.no_of_ffi_calls_threshold_for_gc,
                debug_mode: env.storage.store_context_actions,
            },
            tezos_env.clone(),
            env.enable_testchain,
            &env.storage.tezos_data_dir,
            &env.ffi.protocol_runner,
            env.logging.level,
            true,
        ),
        log.new(o!("endpoint" => "apply_blocks_protocol_runner_endpoint")),
    ) {
        Ok(endpoint) => endpoint,
        Err(e) => shutdown_and_exit!(
            error!(log, "Failed to configure protocol runner endpoint"; "name" => "apply_blocks_protocol_runner_endpoint", "reason" => format!("{:?}", e)),
            actor_system
        ),
    };
    let (
        apply_blocks_protocol_runner_endpoint_run_feature,
        apply_block_protocol_events,
        apply_block_protocol_commands,
    ) = match apply_blocks_protocol_runner_endpoint.start_in_restarting_mode() {
        Ok(run_feature) => {
            let ProtocolRunnerEndpoint {
                events: apply_block_protocol_events,
                commands: apply_block_protocol_commands,
                ..
            } = apply_blocks_protocol_runner_endpoint;
            (
                run_feature,
                apply_block_protocol_events,
                apply_block_protocol_commands,
            )
        }
        Err(e) => shutdown_and_exit!(
            error!(log, "Failed to spawn protocol runner process"; "name" => "apply_blocks_protocol_runner_endpoint", "reason" => e),
            actor_system
        ),
    };

    let current_mempool_state_storage = init_mempool_state_storage();

    let tokio_runtime = create_tokio_runtime(&env);

    let network_channel =
        NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
    let shell_channel = ShellChannel::actor(&actor_system).expect("Failed to create shell channel");

    // it's important to start ContextListener before ChainFeeder, because chain_feeder can trigger init_genesis which sends ContextAction, and we need to process this action first
    let _ = ContextListener::actor(
        &actor_system,
        &persistent_storage,
        apply_block_protocol_events.expect("Context listener needs event server"),
        log.clone(),
        env.storage.store_context_actions,
    )
    .expect("Failed to create context event listener");
    let block_applier = ChainFeeder::actor(
        &actor_system,
        shell_channel.clone(),
        &persistent_storage,
        &init_storage_data,
        &tezos_env,
        apply_block_protocol_commands,
        log.clone(),
    )
    .expect("Failed to create chain feeder");
    let _ = ChainManager::actor(
        &actor_system,
        block_applier,
        network_channel.clone(),
        shell_channel.clone(),
        persistent_storage.clone(),
        tezos_readonly_prevalidation_api_pool.clone(),
        init_storage_data.chain_id.clone(),
        is_sandbox,
        current_mempool_state_storage.clone(),
        env.p2p.disable_mempool,
        &env.p2p.peer_threshold,
        identity.clone(),
    )
    .expect("Failed to create chain manager");

    if env.p2p.disable_mempool {
        info!(log, "Mempool disabled");
    } else {
        info!(log, "Mempool enabled");
        let _ = MempoolPrevalidator::actor(
            &actor_system,
            shell_channel.clone(),
            &persistent_storage,
            current_mempool_state_storage.clone(),
            init_storage_data.chain_id.clone(),
            tezos_readonly_api_pool.clone(),
            log.clone(),
        )
        .expect("Failed to create mempool prevalidator");
    }
    // and than open p2p and others
    let _ = PeerManager::actor(
        &actor_system,
        network_channel.clone(),
        shell_channel.clone(),
        tokio_runtime.handle().clone(),
        identity,
        shell_compatibility_version.clone(),
        env.p2p.clone(),
    )
    .expect("Failed to create peer manager");
    let websocket_handler =
        WebsocketHandler::actor(&actor_system, env.rpc.websocket_address, log.clone())
            .expect("Failed to start websocket actor");
    let _ = Monitor::actor(
        &actor_system,
        network_channel,
        websocket_handler,
        shell_channel.clone(),
    )
    .expect("Failed to create monitor actor");
    let _ = RpcServer::actor(
        &actor_system,
        shell_channel.clone(),
        ([0, 0, 0, 0], env.rpc.listener_port).into(),
        &tokio_runtime.handle(),
        &persistent_storage,
        current_mempool_state_storage,
        &tezedge_context,
        tezos_readonly_api_pool.clone(),
        tezos_readonly_prevalidation_api_pool.clone(),
        tezos_without_context_api_pool.clone(),
        tezos_env.clone(),
        Arc::new(shell_compatibility_version.to_network_version()),
        &init_storage_data,
        is_sandbox,
    )
    .expect("Failed to create RPC server");

    tokio_runtime.block_on(async move {
        use tokio::signal;
        use tokio::time::timeout;

        signal::ctrl_c()
            .await
            .expect("Failed to listen for ctrl-c event");
        info!(log, "Ctrl-c or SIGINT received!");

        // disable/stop protocol runner for applying blocks feature
        let (
            apply_blocks_protocol_runner_endpoint_run_feature,
            apply_blocks_protocol_runner_endpoint_watchdog_thread,
        ) = apply_blocks_protocol_runner_endpoint_run_feature;
        // stop restarting feature
        apply_blocks_protocol_runner_endpoint_run_feature.store(false, Ordering::Release);

        info!(log, "Sending shutdown notification to actors (1/5)");
        shell_channel.tell(
            Publish {
                msg: ShuttingDown.into(),
                topic: ShellChannelTopic::ShellShutdown.into(),
            },
            None,
        );

        // give actors some time to shut down
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!(log, "Shutting down actors (2/5)");
        match timeout(Duration::from_secs(10), actor_system.shutdown()).await {
            Ok(_) => info!(log, "Shutdown actors complete"),
            Err(_) => info!(log, "Shutdown actors did not finish to timeout (10s)"),
        };

        info!(log, "Shutting down protocol runner pools (3/5)");
        drop(tezos_readonly_api_pool);
        drop(tezos_readonly_prevalidation_api_pool);
        drop(tezos_without_context_api_pool);
        if let Ok(mut protocol_runner_process) =
            apply_blocks_protocol_runner_endpoint_watchdog_thread.join()
        {
            if let Err(e) = ExecutableProtocolRunner::wait_and_terminate_ref(
                &mut protocol_runner_process,
                Duration::from_secs(2),
            ) {
                warn!(log, "Failed to terminate/kill protocol runner"; "reason" => e);
            }
        };
        debug!(log, "Protocol runners completed");

        info!(log, "Flushing databases (4/5)");
        drop(persistent_storage);
        info!(log, "Databases flushed");

        info!(log, "Shutdown complete (5/5)");
    });
}

fn main() {
    // Parses config + cli args
    let env = crate::configuration::Environment::from_args();
    let tezos_env = environment::TEZOS_ENV
        .get(&env.tezos_network)
        .unwrap_or_else(|| {
            panic!(
                "No tezos environment version configured for: {:?}",
                env.tezos_network
            )
        });

    // Creates default logger
    let log = create_logger(&env);

    // Loads tezos identity based on provided identity-file argument. In case it does not exist, it will try to automatically generate it
    let tezos_identity = match identity::ensure_identity(&env.identity, &log) {
        Ok(identity) => {
            info!(log, "Identity loaded from file"; "file" => env.identity.identity_json_file_path.as_path().display().to_string());
            if env.validate_cfg_identity_and_stop {
                info!(log, "Configuration and identity is ok!");
                return;
            }
            identity
        }
        Err(e) => {
            error!(log, "Failed to load identity"; "reason" => format!("{}", e), "file" => env.identity.identity_json_file_path.as_path().display().to_string());
            panic!(
                "Failed to load identity: {}",
                env.identity
                    .identity_json_file_path
                    .as_path()
                    .display()
                    .to_string()
            );
        }
    };

    // Enable core dumps and increase open files limit
    system::init_limits(&log);

    let actor_system = SystemBuilder::new()
        .name("light-node")
        .log(log.clone())
        .create()
        .expect("Failed to create actor system");

    // create common RocksDB block cache to be shared among column families
    // IMPORTANT: Cache object must live at least as long as DB (returned by open_kv)
    let cache = Cache::new_lru_cache(128 * 1024 * 1024).unwrap(); // 128 MB

    let schemas = vec![
        block_storage::BlockPrimaryIndex::descriptor(&cache),
        block_storage::BlockByLevelIndex::descriptor(&cache),
        block_storage::BlockByContextHashIndex::descriptor(&cache),
        BlockMetaStorage::descriptor(&cache),
        OperationsStorage::descriptor(&cache),
        OperationsMetaStorage::descriptor(&cache),
        context_action_storage::ContextActionByBlockHashIndex::descriptor(&cache),
        context_action_storage::ContextActionByContractIndex::descriptor(&cache),
        context_action_storage::ContextActionByTypeIndex::descriptor(&cache),
        ContextActionStorage::descriptor(&cache),
        MerkleStorage::descriptor(&cache),
        SystemStorage::descriptor(&cache),
        Sequences::descriptor(&cache),
        MempoolStorage::descriptor(&cache),
        ChainMetaStorage::descriptor(&cache),
        PredecessorStorage::descriptor(&cache),
    ];

    let rocks_db = match open_kv(&env.storage.db_path, schemas, &env.storage.db_cfg) {
        Ok(db) => Arc::new(db),
        Err(e) => shutdown_and_exit!(
            error!(log, "Failed to create RocksDB database at '{:?}'", &env.storage.db_path; "reason" => e),
            actor_system
        ),
    };
    debug!(log, "Loaded RocksDB database");

    match check_database_compatibility(rocks_db.clone(), DATABASE_VERSION, &tezos_env, &log) {
        Ok(false) => shutdown_and_exit!(
            crit!(log, "Database incompatibility detected"),
            actor_system
        ),
        Err(e) => shutdown_and_exit!(
            error!(log, "Failed to verify database compatibility"; "reason" => e),
            actor_system
        ),
        _ => (),
    }

    let schemas = vec![BlockStorage::descriptor()];

    {
        let commit_logs = match open_cl(&env.storage.db_path, schemas) {
            Ok(commit_logs) => Arc::new(commit_logs),
            Err(e) => shutdown_and_exit!(
                error!(log, "Failed to open commit logs"; "reason" => e),
                actor_system
            ),
        };

        let persistent_storage = PersistentStorage::new(rocks_db, commit_logs);
        let tezedge_context = TezedgeContext::new(
            BlockStorage::new(&persistent_storage),
            persistent_storage.merkle(),
        );
        match resolve_storage_init_chain_data(
            &tezos_env,
            &env.storage.db_path,
            &env.storage.tezos_data_dir,
            &env.storage.patch_context,
            &log,
        ) {
            Ok(init_data) => block_on_actors(
                env,
                tezos_env,
                init_data,
                Arc::new(tezos_identity),
                actor_system,
                persistent_storage,
                tezedge_context,
                log,
            ),
            Err(e) => shutdown_and_exit!(
                error!(log, "Failed to resolve init storage chain data."; "reason" => e),
                actor_system
            ),
        }
    }
}
