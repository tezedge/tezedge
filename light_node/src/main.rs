// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use riker::actors::*;
use slog::{crit, debug, Drain, error, info, Logger};

use logging::detailed_json;
use logging::file::FileAppenderBuilder;
use monitoring::{Monitor, WebsocketHandler};
use networking::p2p::network_channel::NetworkChannel;
use rpc::rpc_actor::RpcServer;
use shell::chain_feeder::ChainFeeder;
use shell::chain_manager::ChainManager;
use shell::context_listener::ContextListener;
use shell::mempool_prevalidator::MempoolPrevalidator;
use shell::peer_manager::PeerManager;
use shell::shell_channel::{ShellChannel, ShellChannelTopic, ShuttingDown};
use storage::{block_storage, BlockMetaStorage, BlockStorage, ChainMetaStorage, check_database_compatibility, context_action_storage, ContextActionStorage, MempoolStorage, OperationsMetaStorage, OperationsStorage, resolve_storage_init_chain_data, StorageInitInfo, SystemStorage};
use storage::persistent::{CommitLogSchema, KeyValueSchema, open_cl, open_kv, PersistentStorage};
use storage::persistent::sequence::Sequences;
use storage::skip_list::{DatabaseBackedSkipList, Lane, ListValue};
use tezos_api::environment;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_api::identity::Identity;
use tezos_messages::p2p::encoding::version::NetworkVersion;
use tezos_wrapper::{TezosApiConnectionPool, TezosApiConnectionPoolConfiguration};
use tezos_wrapper::service::{ExecutableProtocolRunner, ProtocolEndpointConfiguration, ProtocolRunnerEndpoint};

use crate::configuration::LogFormat;

mod configuration;
mod identity;
mod system;

const DATABASE_VERSION: i64 = 15;
const SUPPORTED_DISTRIBUTED_DB_VERSION: u16 = 0;
const SUPPORTED_P2P_VERSION: u16 = 1;

macro_rules! shutdown_and_exit {
    ($err:expr, $sys:ident) => {{
        $err;
        futures::executor::block_on($sys.shutdown()).unwrap();
        return;
    }}
}

macro_rules! create_terminal_logger {
    ($type:expr) => {{
        match $type {
            LogFormat::Simple => slog_async::Async::new(slog_term::FullFormat::new(slog_term::TermDecorator::new().build()).build().fuse()).chan_size(32768).overflow_strategy(slog_async::OverflowStrategy::Block).build(),
            LogFormat::Json => slog_async::Async::new(detailed_json::default(std::io::stdout()).fuse()).chan_size(32768).overflow_strategy(slog_async::OverflowStrategy::Block).build(),
        }
    }}
}

macro_rules! create_file_logger {
    ($type:expr, $path:expr) => {{
        let appender = FileAppenderBuilder::new($path)
            .rotate_size(10_485_760) // 10 MB
            .rotate_keep(4)
            .rotate_compress(true)
            .build();

        match $type {
            LogFormat::Simple => slog_async::Async::new(slog_term::FullFormat::new(slog_term::PlainDecorator::new(appender)).build().fuse()).chan_size(32768).overflow_strategy(slog_async::OverflowStrategy::Block).build(),
            LogFormat::Json => slog_async::Async::new(detailed_json::default(appender).fuse()).chan_size(32768).overflow_strategy(slog_async::OverflowStrategy::Block).build(),
        }
    }}
}

fn create_logger(env: &crate::configuration::Environment) -> Logger {
    let drain = match &env.logging.file {
        Some(log_file) => create_file_logger!(env.logging.format, log_file),
        None => create_terminal_logger!(env.logging.format),
    }.filter_level(env.logging.level).fuse();

    Logger::root(drain, slog::o!())
}

fn create_tokio_runtime(env: &crate::configuration::Environment) -> tokio::runtime::Runtime {
    let mut builder = tokio::runtime::Builder::new();
    // use threaded work staling scheduler
    builder.threaded_scheduler().enable_all();
    // set number of threads in a thread pool
    if env.tokio_threads > 0 {
        builder.core_threads(env.tokio_threads);
    }
    // build runtime
    builder.build().expect("Failed to create tokio runtime")
}

/// Create pool for ffi protocol runner connections (used just for readonly context)
/// Connections are created on demand, but depends on [TezosApiConnectionPoolConfiguration][min_connections]
fn create_tezos_readonly_api_pool(env: &crate::configuration::Environment, tezos_env: TezosEnvironmentConfiguration, log: Logger) -> TezosApiConnectionPool {
    TezosApiConnectionPool::new_with_readonly_context(
        String::from("tezos_readonly_api_pool"),
        env.ffi.pool.clone(),
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
fn create_tezos_writeable_api_pool(env: &crate::configuration::Environment, tezos_env: TezosEnvironmentConfiguration, log: Logger) -> TezosApiConnectionPool {
    TezosApiConnectionPool::new_without_context(
        String::from("tezos_writeable_api_pool"),
        TezosApiConnectionPoolConfiguration {
            idle_timeout: env.ffi.pool.idle_timeout,
            max_lifetime: env.ffi.pool.max_lifetime,
            connection_timeout: env.ffi.pool.connection_timeout,
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
    identity: Identity,
    actor_system: ActorSystem,
    persistent_storage: PersistentStorage,
    log: Logger) {

    // if feeding is started, than run chain manager
    let is_sandbox = env.tezos_network == environment::TezosEnvironment::Sandbox;
    // version
    let network_version = NetworkVersion::new(
        tezos_env.version.clone(),
        SUPPORTED_DISTRIBUTED_DB_VERSION,
        SUPPORTED_P2P_VERSION,
    );

    // create pool for ffi protocol runner connections (used just for readonly context)
    let tezos_readonly_api = Arc::new(create_tezos_readonly_api_pool(&env, tezos_env.clone(), log.clone()));

    // tezos protocol runner endpoint for applying blocks to chain
    let mut apply_blocks_protocol_runner_endpoint = ProtocolRunnerEndpoint::<ExecutableProtocolRunner>::new(
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
        log.clone(),
    );
    let (apply_blocks_protocol_runner_endpoint_run_feature, apply_block_protocol_events, apply_block_protocol_commands) = match apply_blocks_protocol_runner_endpoint.start_in_restarting_mode() {
        Ok(run_feature) => {
            info!(log, "Protocol runner started successfully"; "endpoint" => apply_blocks_protocol_runner_endpoint.name);
            let ProtocolRunnerEndpoint {
                events: apply_block_protocol_events,
                commands: apply_block_protocol_commands,
                ..
            } = apply_blocks_protocol_runner_endpoint;
            (run_feature, apply_block_protocol_events, apply_block_protocol_commands)
        }
        Err(e) => shutdown_and_exit!(error!(log, "Failed to spawn protocol runner process"; "name" => apply_blocks_protocol_runner_endpoint.name, "reason" => e), actor_system),
    };

    let mut tokio_runtime = create_tokio_runtime(&env);

    let network_channel = NetworkChannel::actor(&actor_system)
        .expect("Failed to create network channel");
    let shell_channel = ShellChannel::actor(&actor_system)
        .expect("Failed to create shell channel");

    // it's important to start ContextListener before ChainFeeder, because chain_feeder can trigger init_genesis which sends ContextAction, and we need to process this action first
    let _ = ContextListener::actor(&actor_system, &persistent_storage, apply_block_protocol_events.expect("Context listener needs event server"), log.clone(), env.storage.store_context_actions)
        .expect("Failed to create context event listener");
    let _ = ChainFeeder::actor(&actor_system, shell_channel.clone(), &persistent_storage, &init_storage_data, &tezos_env, apply_block_protocol_commands, log.clone())
        .expect("Failed to create chain feeder");
    let _ = ChainManager::actor(&actor_system, network_channel.clone(), shell_channel.clone(), &persistent_storage, &init_storage_data.chain_id, is_sandbox)
        .expect("Failed to create chain manager");

    let _ = MempoolPrevalidator::actor(
        &actor_system,
        shell_channel.clone(),
        &persistent_storage,
        &init_storage_data,
        tezos_readonly_api.clone(),
        log.clone(),
    ).expect("Failed to create chain feeder");

    // and than open p2p and others
    let _ = PeerManager::actor(
        &actor_system,
        network_channel.clone(),
        shell_channel.clone(),
        tokio_runtime.handle().clone(),
        identity,
        network_version.clone(),
        env.p2p,
    ).expect("Failed to create peer manager");
    let websocket_handler = WebsocketHandler::actor(&actor_system, env.rpc.websocket_address, log.clone())
        .expect("Failed to start websocket actor");
    let _ = Monitor::actor(&actor_system, network_channel.clone(), websocket_handler, shell_channel.clone(), &persistent_storage, &init_storage_data)
        .expect("Failed to create monitor actor");
    let _ = RpcServer::actor(
        &actor_system,
        shell_channel.clone(),
        ([0, 0, 0, 0], env.rpc.listener_port).into(),
        &tokio_runtime.handle(),
        &persistent_storage,
        tezos_readonly_api.clone(),
        tezos_env.clone(),
        network_version,
        &init_storage_data,
    ).expect("Failed to create RPC server");

    tokio_runtime.block_on(async move {
        use tokio::signal;

        signal::ctrl_c().await.expect("Failed to listen for ctrl-c event");
        info!(log, "ctrl-c received!");

        // disable/stop protocol runner for applying blocks feature
        apply_blocks_protocol_runner_endpoint_run_feature.store(false, Ordering::Release);

        info!(log, "Sending shutdown notification to actors");
        shell_channel.tell(
            Publish {
                msg: ShuttingDown.into(),
                topic: ShellChannelTopic::ShellCommands.into(),
            }, None,
        );

        // give actors some time to shut down
        thread::sleep(Duration::from_secs(1));


        info!(log, "Shutting down actors");
        let _ = actor_system.shutdown().await;
        info!(log, "Shutdown actors complete");

        thread::sleep(Duration::from_secs(1));

        info!(log, "Shutting down protocol runner pools");
        drop(tezos_readonly_api);
        debug!(log, "Shutdown tezos_readonly_api complete");

        info!(log, "Shutdown complete");
    });
}

fn main() {
    // Parses config + cli args
    let env = crate::configuration::Environment::from_args();
    let tezos_env = environment::TEZOS_ENV
        .get(&env.tezos_network)
        .expect(&format!("No tezos environment version configured for: {:?}", env.tezos_network));

    // Creates default logger
    let log = create_logger(&env);

    // Enable core dumps and increase open files limit
    system::init_limits(&log);

    let actor_system = SystemBuilder::new().name("light-node").log(log.clone()).create().expect("Failed to create actor system");

    // Loads tezos identity based on provided identity-file argument. In case it does not exist, it will try to automatically generate it
    let tezos_identity = match identity::ensure_identity(&env.identity, &log) {
        Ok(identity) => {
            info!(log, "Identity loaded from file"; "file" => env.identity.identity_json_file_path.clone().into_os_string().into_string().unwrap());
            identity
        }
        Err(e) => shutdown_and_exit!(error!(log, "Failed to load identity"; "reason" => e, "file" => env.identity.identity_json_file_path.into_os_string().into_string().unwrap()), actor_system),
    };

    let schemas = vec![
        block_storage::BlockPrimaryIndex::descriptor(),
        block_storage::BlockByLevelIndex::descriptor(),
        block_storage::BlockByContextHashIndex::descriptor(),
        BlockMetaStorage::descriptor(),
        OperationsStorage::descriptor(),
        OperationsMetaStorage::descriptor(),
        context_action_storage::ContextActionByBlockHashIndex::descriptor(),
        context_action_storage::ContextActionByContractIndex::descriptor(),
        context_action_storage::ContextActionByTypeIndex::descriptor(),
        ContextActionStorage::descriptor(),
        SystemStorage::descriptor(),
        DatabaseBackedSkipList::descriptor(),
        Lane::descriptor(),
        ListValue::descriptor(),
        Sequences::descriptor(),
        MempoolStorage::descriptor(),
        ChainMetaStorage::descriptor(),
    ];
    let rocks_db = match open_kv(&env.storage.db_path, schemas, &env.storage.db_cfg) {
        Ok(db) => Arc::new(db),
        Err(_) => shutdown_and_exit!(error!(log, "Failed to create RocksDB database at '{:?}'", &env.storage.db_path), actor_system)
    };
    debug!(log, "Loaded RocksDB database");

    match check_database_compatibility(rocks_db.clone(), DATABASE_VERSION, &tezos_env, &log) {
        Ok(false) => shutdown_and_exit!(crit!(log, "Database incompatibility detected"), actor_system),
        Err(e) => shutdown_and_exit!(error!(log, "Failed to verify database compatibility"; "reason" => e), actor_system),
        _ => ()
    }

    let schemas = vec![
        BlockStorage::descriptor()
    ];

    {
        let commit_logs = match open_cl(&env.storage.db_path, schemas) {
            Ok(commit_logs) => Arc::new(commit_logs),
            Err(e) => shutdown_and_exit!(error!(log, "Failed to open commit logs"; "reason" => e), actor_system)
        };

        let persistent_storage = PersistentStorage::new(rocks_db, commit_logs);
        match resolve_storage_init_chain_data(
            &tezos_env,
            &env.storage.db_path,
            &env.storage.tezos_data_dir,
            &env.storage.patch_context,
            &log) {
            Ok(init_data) => block_on_actors(env, tezos_env, init_data, tezos_identity, actor_system, persistent_storage, log),
            Err(e) => shutdown_and_exit!(error!(log, "Failed to resolve init storage chain data. Reason: {}", e), actor_system),
        }
    }
}
