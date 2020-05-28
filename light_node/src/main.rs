// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;
use std::sync::atomic::Ordering;
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
use shell::peer_manager::PeerManager;
use shell::shell_channel::{ShellChannel, ShellChannelTopic, ShuttingDown};
use storage::{block_storage, BlockMetaStorage, BlockStorage, context_action_storage, ContextActionStorage, MempoolStorage, OperationsMetaStorage, OperationsStorage, resolve_storage_init_chain_data, StorageError, StorageInitInfo, SystemStorage};
use storage::p2p_message_storage::{P2PMessageSecondaryIndex, P2PMessageStorage};
use storage::persistent::{CommitLogSchema, KeyValueSchema, open_cl, open_kv, PersistentStorage};
use storage::persistent::sequence::Sequences;
use storage::skip_list::{DatabaseBackedSkipList, Lane, ListValue};
use tezos_api::environment;
use tezos_api::environment::TezosEnvironmentConfiguration;
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_api::identity::Identity;
use tezos_wrapper::service::{ProtocolEndpointConfiguration, ProtocolRunnerEndpoint, ExecutableProtocolRunner};

use crate::configuration::LogFormat;

mod configuration;
mod identity;

const DATABASE_VERSION: i64 = 13;

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

fn block_on_actors(
    env: &crate::configuration::Environment,
    tezos_env: &TezosEnvironmentConfiguration,
    init_storage_data: StorageInitInfo,
    identity: Identity,
    actor_system: ActorSystem,
    persistent_storage: PersistentStorage,
    log: Logger) {

    // tezos protocol runner endpoint for applying blocks to chain
    let mut apply_blocks_protocol_runner_endpoint = ProtocolRunnerEndpoint::<ExecutableProtocolRunner>::new(
        "apply_blocks_protocol_runner_endpoint",
        ProtocolEndpointConfiguration::new(
            TezosRuntimeConfiguration {
                log_enabled: env.logging.ocaml_log_enabled,
                no_of_ffi_calls_treshold_for_gc: env.no_of_ffi_calls_threshold_for_gc,
                debug_mode: env.storage.store_context_actions,
            },
            tezos_env.clone(),
            env.enable_testchain,
            &env.storage.tezos_data_dir,
            &env.protocol_runner,
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

    let mut tokio_runtime = create_tokio_runtime(env);

    let network_channel = NetworkChannel::actor(&actor_system)
        .expect("Failed to create network channel");
    let shell_channel = ShellChannel::actor(&actor_system)
        .expect("Failed to create shell channel");

    // it's important to start ContextListener before ChainFeeder, because chain_feeder can trigger init_genesis which sends ContextAction, and we need to process this action first
    let _ = ContextListener::actor(&actor_system, &persistent_storage, apply_block_protocol_events, log.clone(), env.storage.store_context_actions)
        .expect("Failed to create context event listener");
    let _ = ChainFeeder::actor(&actor_system, shell_channel.clone(), &persistent_storage, &init_storage_data, &tezos_env, apply_block_protocol_commands, log.clone())
        .expect("Failed to create chain feeder");
    // if feeding is started, than run chain manager
    let _ = ChainManager::actor(&actor_system, network_channel.clone(), shell_channel.clone(), &persistent_storage, &init_storage_data.chain_id, env.storage.store_p2p_messages)
        .expect("Failed to create chain manager");

    // and than open p2p and others
    let _ = PeerManager::actor(
        &actor_system,
        network_channel.clone(),
        shell_channel.clone(),
        tokio_runtime.handle().clone(),
        &env.p2p.bootstrap_lookup_addresses,
        &env.p2p.initial_peers,
        env.p2p.peer_threshold,
        env.p2p.listener_port,
        identity,
        tezos_env.version.clone(),
        persistent_storage.clone(),
        env.storage.store_p2p_messages)
        .expect("Failed to create peer manager");
    let websocket_handler = WebsocketHandler::actor(&actor_system, env.rpc.websocket_address, log.clone())
        .expect("Failed to start websocket actor");
    let _ = Monitor::actor(&actor_system, network_channel.clone(), websocket_handler, shell_channel.clone(), &persistent_storage)
        .expect("Failed to create monitor actor");
    let _ = RpcServer::actor(&actor_system, shell_channel.clone(), ([0, 0, 0, 0], env.rpc.listener_port).into(), &tokio_runtime.handle(), &persistent_storage, &init_storage_data)
        .expect("Failed to create RPC server");

    tokio_runtime.block_on(async move {
        use std::thread;
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
        info!(log, "Shutdown complete");
    });

    tokio_runtime.shutdown_timeout(Duration::from_millis(100));
}

fn check_database_compatibility(db: Arc<rocksdb::DB>, tezos_env: &TezosEnvironmentConfiguration, log: Logger) -> Result<bool, StorageError> {
    let mut system_info = SystemStorage::new(db.clone());
    let db_version_ok = match system_info.get_db_version()? {
        Some(db_version) => db_version == DATABASE_VERSION,
        None => {
            system_info.set_db_version(DATABASE_VERSION)?;
            true
        }
    };
    if !db_version_ok {
        error!(log, "Incompatible database version found. Please re-sync your node to empty storage - see configuration!");
    }

    let tezos_env_main_chain = tezos_env.main_chain_id().map_err(|e| StorageError::TezosEnvironmentError { error: e })?;

    let (chain_id_ok, previous, requested) = match system_info.get_chain_id()? {
        Some(chain_id) => {
            // find previos chain_name
            let previous = environment::TEZOS_ENV
                .iter()
                .find(|(_, v)| {
                    if let Ok(cid) = v.main_chain_id() {
                        cid == chain_id
                    } else {
                        false
                    }
                })
                .map_or("-unknown-", |(_, v)| &v.version);

            if chain_id == tezos_env_main_chain {
                (true, previous, &tezos_env.version)
            } else {
                (false, previous, &tezos_env.version)
            }
        }
        None => {
            system_info.set_chain_id(&tezos_env_main_chain)?;
            (true, "-none-", &tezos_env.version)
        }
    };

    if !chain_id_ok {
        error!(log, "Current database was previously created for another chain. Please re-sync your node to empty storage - see configuration!"; "requested" => requested, "previous" => previous);
    }

    Ok(db_version_ok && chain_id_ok)
}

fn main() {
    // Parses config + cli args
    let env = crate::configuration::Environment::from_args();
    let tezos_env = environment::TEZOS_ENV
        .get(&env.tezos_network)
        .expect(&format!("No tezos environment version configured for: {:?}", env.tezos_network));

    // Creates default logger
    let log = create_logger(&env);

    let actor_system = SystemBuilder::new().name("light-node").log(log.clone()).create().expect("Failed to create actor system");

    // Loads tezos identity based on provided identity-file argument. In case it does not exist, it will try to automatically generate it
    let tezos_identity = match identity::ensure_identity(&env.identity, log.clone()) {
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
        context_action_storage::ContextActionPrimaryIndex::descriptor(),
        context_action_storage::ContextActionByContractIndex::descriptor(),
        SystemStorage::descriptor(),
        DatabaseBackedSkipList::descriptor(),
        P2PMessageStorage::descriptor(),
        P2PMessageSecondaryIndex::descriptor(),
        Lane::descriptor(),
        ListValue::descriptor(),
        Sequences::descriptor(),
        MempoolStorage::descriptor(),
    ];
    let rocks_db = match open_kv(&env.storage.bootstrap_db_path, schemas) {
        Ok(db) => Arc::new(db),
        Err(_) => shutdown_and_exit!(error!(log, "Failed to create RocksDB database at '{:?}'", &env.storage.bootstrap_db_path), actor_system)
    };
    debug!(log, "Loaded RocksDB database");

    match check_database_compatibility(rocks_db.clone(), &tezos_env, log.clone()) {
        Ok(false) => shutdown_and_exit!(crit!(log, "Database incompatibility detected"), actor_system),
        Err(e) => shutdown_and_exit!(error!(log, "Failed to verify database compatibility"; "reason" => e), actor_system),
        _ => ()
    }

    let schemas = vec![
        BlockStorage::descriptor(),
        ContextActionStorage::descriptor()
    ];

    {
        let commit_logs = match open_cl(&env.storage.bootstrap_db_path, schemas) {
            Ok(commit_logs) => Arc::new(commit_logs),
            Err(e) => shutdown_and_exit!(error!(log, "Failed to open commit logs"; "reason" => e), actor_system)
        };

        let persistent_storage = PersistentStorage::new(rocks_db, commit_logs);
        match resolve_storage_init_chain_data(&tezos_env, &env.storage.bootstrap_db_path, &env.storage.tezos_data_dir, log.clone()) {
            Ok(init_data) => block_on_actors(&env, tezos_env, init_data, tezos_identity, actor_system, persistent_storage, log),
            Err(e) => shutdown_and_exit!(error!(log, "Failed to resolve init storage chain data. Reason: {}", e), actor_system),
        }
    }
}
