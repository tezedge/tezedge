// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
// NOTE: unsafe cannot be forbidden right now because of code in systems.rs
// #![forbid(unsafe_code)]

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use slog::{debug, error, info, warn, Logger};
use tezedge_actor_system::actors::*;

use crypto::hash::BlockHash;
use monitoring::{Monitor, WebsocketHandler};
use networking::p2p::network_channel::NetworkChannel;
use networking::ShellCompatibilityVersion;
use rpc::RpcServer;
use shell::chain_feeder::ApplyBlock;
use shell::chain_feeder::ChainFeederRef;
use shell::chain_manager::{ChainManager, ChainManagerRef};
use shell::connector::ShellConnectorSupport;
use shell::mempool::{init_mempool_state_storage, MempoolPrevalidatorFactory};
use shell::peer_manager::PeerManager;
use shell::shell_channel::ShellChannelRef;
use shell::shell_channel::{ShellChannel, ShellChannelTopic, ShuttingDown};
use shell::{chain_feeder::ChainFeeder, state::ApplyBlockBatch};
use shell_integration::{create_oneshot_callback, ThreadWatcher};
use storage::persistent::sequence::Sequences;
use storage::persistent::{open_cl, CommitLogSchema};
use storage::{
    hydrate_current_head, resolve_storage_init_chain_data, BlockHeaderWithHash, BlockStorage,
    PersistentStorage, StorageInitInfo,
};
use storage::{
    initializer::{initialize_rocksdb, GlobalRocksDbCacheHolder, MainChain, RocksDbCache},
    BlockMetaStorage, Replay,
};
use tezos_api::environment;
use tezos_api::ffi::TezosRuntimeConfiguration;
use tezos_identity::Identity;
use tezos_messages::Head;
use tezos_protocol_ipc_client::{
    ProtocolRunnerApi, ProtocolRunnerConfiguration, ProtocolRunnerInstance,
};

use crate::configuration::Environment;
use crate::notification_integration::RpcNotificationCallbackActor;
use storage::database::tezedge_database::TezedgeDatabaseBackendConfiguration;
use storage::initializer::initialize_maindb;

mod configuration;
mod identity;
mod notification_integration;
mod system;

extern crate jemallocator;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn create_tokio_runtime(
    env: &crate::configuration::Environment,
) -> std::io::Result<tokio::runtime::Runtime> {
    // use threaded work staling scheduler
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    // set number of threads in a thread pool
    if env.tokio_threads > 0 {
        builder.worker_threads(env.tokio_threads);
    }
    // build runtime
    builder.build()
}

fn create_protocol_runner_configuration(
    env: &crate::configuration::Environment,
) -> ProtocolRunnerConfiguration {
    ProtocolRunnerConfiguration::new(
        TezosRuntimeConfiguration {
            log_enabled: env.logging.ocaml_log_enabled,
            debug_mode: false,
            compute_context_action_tree_hashes: false,
        },
        env.tezos_network_config.clone(),
        env.enable_testchain,
        env.storage.context_storage_configuration.clone(),
        env.ffi.protocol_runner.clone(),
        env.logging.slog.level,
    )
}

/// Create pool for ffi protocol runner connections (used just for readonly context)
/// Connections are created on demand, but depends on [TezosApiConnectionPoolConfiguration][min_connections]
//fn create_tezos_readonly_api_pool(
//    pool_name: &str,
//    pool_cfg: TezosApiConnectionPoolConfiguration,
//    env: &crate::configuration::Environment,
//    tokio_runtime: tokio::runtime::Handle,
//    log: Logger,
//) -> Result<TezosApiConnectionPool, TezosApiConnectionPoolError> {
//    TezosApiConnectionPool::new_with_readonly_context(
//        String::from(pool_name),
//        pool_cfg,
//        ProtocolRunnerConfiguration::new(
//            TezosRuntimeConfiguration {
//                log_enabled: env.logging.ocaml_log_enabled,
//                debug_mode: false,
//                compute_context_action_tree_hashes: false,
//            },
//            env.tezos_network_config.clone(),
//            env.enable_testchain,
//            env.storage.context_storage_configuration.readonly(),
//            env.ffi.protocol_runner.clone(),
//            env.logging.slog.level,
//        ),
//        tokio_runtime,
//        log,
//    )
//}

/// Create pool for ffi protocol runner connections (used just for ffi calls which does not need context)
/// Connections are created on demand, but depends on [TezosApiConnectionPoolConfiguration][min_connections]
//fn create_tezos_without_context_api_pool(
//    pool_name: &str,
//    pool_cfg: TezosApiConnectionPoolConfiguration,
//    env: &crate::configuration::Environment,
//    tokio_runtime: tokio::runtime::Handle,
//    log: Logger,
//) -> Result<TezosApiConnectionPool, TezosApiConnectionPoolError> {
//    TezosApiConnectionPool::new_without_context(
//        String::from(pool_name),
//        pool_cfg,
//        ProtocolRunnerConfiguration::new(
//            TezosRuntimeConfiguration {
//                log_enabled: env.logging.ocaml_log_enabled,
//                debug_mode: false,
//                compute_context_action_tree_hashes: false,
//            },
//            env.tezos_network_config.clone(),
//            env.enable_testchain,
//            env.storage.context_storage_configuration.clone(),
//            &env.ffi.protocol_runner,
//            env.logging.slog.level,
//        ),
//        tokio_runtime,
//        log,
//    )
//}

/// Create pool for ffi protocol runner connection (used for write to context)
/// There is limitation, that only one write connection to context can be open, so we limit this pool to 1.
//fn create_tezos_writeable_api_pool(
//    env: &crate::configuration::Environment,
//    tokio_runtime: tokio::runtime::Handle,
//    log: Logger,
//) -> Result<TezosApiConnectionPool, TezosApiConnectionPoolError> {
//    TezosApiConnectionPool::new_without_context(
//        String::from("tezos_writeable_api_pool"),
//        TezosApiConnectionPoolConfiguration {
//            // TODO: hard-coded, not used, make as Optional
//            idle_timeout: Duration::from_secs(1800),
//            // TODO: hard-coded, not used, make as Optional
//            max_lifetime: Duration::from_secs(21600),
//            connection_timeout: Duration::from_secs(30),
//            min_connections: 0,
//            max_connections: 1,
//        },
//        ProtocolRunnerConfiguration::new(
//            TezosRuntimeConfiguration {
//                log_enabled: env.logging.ocaml_log_enabled,
//                compute_context_action_tree_hashes: env.storage.compute_context_action_tree_hashes,
//                debug_mode: false,
//            },
//            env.tezos_network_config.clone(),
//            env.enable_testchain,
//            env.storage.context_storage_configuration.clone(),
//            &env.ffi.protocol_runner,
//            env.logging.slog.level,
//        ),
//        tokio_runtime,
//        log,
//    )
//}

fn block_on_actors(
    env: crate::configuration::Environment,
    init_storage_data: StorageInitInfo,
    identity: Arc<Identity>,
    persistent_storage: PersistentStorage,
    mut blocks_replay: Option<Vec<Arc<BlockHash>>>,
    log: Logger,
) {
    // if feeding is started, than run chain manager
    let is_sandbox = env.tezos_network == environment::TezosEnvironment::Sandbox;
    // version
    let shell_compatibility_version = Arc::new(ShellCompatibilityVersion::new(
        env.tezos_network_config.version.clone(),
        shell::SUPPORTED_DISTRIBUTED_DB_VERSION.to_vec(),
        shell::SUPPORTED_P2P_VERSION.to_vec(),
    ));

    info!(log, "Initializing protocol runners... (4/8)");

    // create tokio runtime
    let tokio_runtime = create_tokio_runtime(&env).expect("Failed to create tokio runtime");

    // TODO here: spawn process
    let socket_path = Path::new("/tmp/protocol-runner.sock"); // TODO
    let protocol_runner_configuration = create_protocol_runner_configuration(&env);
    let protocol_runner_instance = ProtocolRunnerInstance::new(
        protocol_runner_configuration,
        socket_path,
        "protocol-runner".into(),
        tokio_runtime.handle(),
    );
    let _child = protocol_runner_instance
        .spawn(log.clone())
        .expect("Failed to launch protocol runner");
    let tezos_protocol_api = Arc::new(ProtocolRunnerApi::new(protocol_runner_instance));

    // TODO: remove
    std::thread::sleep(Duration::from_secs(3));

    // pool and event server dedicated for applying blocks to chain
    //    let tezos_writeable_api_pool = Arc::new(
    //        create_tezos_writeable_api_pool(&env, tokio_runtime.handle().clone(), log.clone())
    //            .expect("Failed to initialize writable API pool"),
    //    );
    //
    //    // create pool for ffi protocol runner connections (used just for readonly context)
    //    let tezos_readonly_api_pool = Arc::new(
    //        create_tezos_readonly_api_pool(
    //            "tezos_readonly_api_pool",
    //            env.ffi.tezos_readonly_api_pool.clone(),
    //            &env,
    //            tokio_runtime.handle().clone(),
    //            log.clone(),
    //        )
    //        .expect("Failed to initialize read-only API pool"),
    //    );
    //    let tezos_readonly_prevalidation_api_pool = Arc::new(
    //        create_tezos_readonly_api_pool(
    //            "tezos_readonly_prevalidation_api",
    //            env.ffi.tezos_readonly_prevalidation_api_pool.clone(),
    //            &env,
    //            tokio_runtime.handle().clone(),
    //            log.clone(),
    //        )
    //        .expect("Failed to initialize read-only prevalidation API pool"),
    //    );
    //    let tezos_without_context_api_pool = Arc::new(
    //        create_tezos_without_context_api_pool(
    //            "tezos_without_context_api_pool",
    //            env.ffi.tezos_without_context_api_pool.clone(),
    //            &env,
    //            tokio_runtime.handle().clone(),
    //            log.clone(),
    //        )
    //        .expect("Failed to initialize API pool without context"),
    //    );

    info!(log, "Protocol runners initialized");

    info!(log, "Initializing actors... (5/8)";
               "shell_compatibility_version" => format!("{:?}", &shell_compatibility_version),
               "is_sandbox" => is_sandbox);

    // create partial (global) states for sharing between threads/actors
    let current_mempool_state_storage = init_mempool_state_storage();

    // create actor system
    let actor_system = Arc::new(
        SystemBuilder::new()
            .name("light-node")
            .log(log.clone())
            .cfg({
                let mut cfg = tezedge_actor_system::load_config();
                // websocket uses [`ctx.schedule`] with cca 1.5s delay,
                // and without websocket, the lowest scheduled delay is about 15s
                // so default 50ms for actor system schedule thread is too low
                cfg.scheduler.frequency_millis = if env.rpc.websocket_cfg.is_some() {
                    500
                } else {
                    5000
                };
                cfg
            })
            .exec(tokio_runtime.handle().clone().into())
            .create()
            .expect("Failed to create actor system"),
    );
    info!(log, "Riker configuration"; "cfg" => actor_system.config());

    let network_channel =
        NetworkChannel::actor(actor_system.as_ref()).expect("Failed to create network channel");
    let shell_channel =
        ShellChannel::actor(actor_system.as_ref()).expect("Failed to create shell channel");
    let mempool_prevalidator_factory = Arc::new(MempoolPrevalidatorFactory::new(
        actor_system.clone(),
        log.clone(),
        persistent_storage.clone(),
        current_mempool_state_storage.clone(),
        Arc::clone(&tezos_protocol_api),
        tokio_runtime.handle(),
        env.p2p.disable_mempool,
    ));

    // start chain_feeder with controlled startup and wait for ok initialized context
    info!(log, "Initializing context... (6/8)");
    let (initialize_context_result_callback, initialize_context_result_callback_receiver) =
        create_oneshot_callback();

    let (block_applier, mut block_applier_thread_watcher) = ChainFeeder::actor(
        actor_system.as_ref(),
        persistent_storage.clone(),
        Arc::clone(&tezos_protocol_api),
        tokio_runtime.handle(),
        init_storage_data.clone(),
        env.tezos_network_config.clone(),
        log.clone(),
        initialize_context_result_callback,
    )
    .expect("Failed to create chain feeder");

    match initialize_context_result_callback_receiver
        .recv_timeout(env.storage.initialize_context_timeout)
    {
        Ok(result) => {
            if let Err(e) = result {
                panic!(
                    "Context was not initialized successfully, so cannot continue, reason: {:?}",
                    e
                )
            }
        }
        Err(e) => {
            panic!("Context was not initialized within {:?} timeout, e.g. try increase [--initialize-context-timeout], reason: {}", env.storage.initialize_context_timeout, e)
        }
    };
    info!(log, "Context initialized (6/8)");

    // load current_head, at least genesis should be stored, if not, just finished, something is wrong
    info!(log, "Hydrating current head... (7/8)");
    let hydrated_current_head_block: Arc<BlockHeaderWithHash> =
        hydrate_current_head(&init_storage_data, &persistent_storage)
            .expect("Failed to load current_head from database");
    let hydrated_current_head = Head::new(
        hydrated_current_head_block.hash.clone(),
        hydrated_current_head_block.header.level(),
        hydrated_current_head_block.header.fitness().clone(),
    );
    {
        let (head, level, fitness) = hydrated_current_head.to_debug_info();
        info!(log, "Current head hydrated (7/8)";
                   "block_hash" => head,
                   "level" => level,
                   "fitness" => fitness);
    }

    // start chain_manager with controlled startup and wait for initialization
    info!(log, "Initializing chain manager... (8/8)");
    let (
        initialize_chain_manager_result_callback,
        initialize_chain_manager_result_callback_receiver,
    ) = create_oneshot_callback();

    let (chain_manager, mut chain_manager_p2p_reader_thread_watcher) = ChainManager::actor(
        actor_system.as_ref(),
        block_applier.clone(),
        network_channel.clone(),
        shell_channel.clone(),
        persistent_storage.clone(),
        Arc::clone(&tezos_protocol_api),
        init_storage_data.clone(),
        is_sandbox,
        hydrated_current_head,
        current_mempool_state_storage.clone(),
        env.p2p
            .peer_threshold
            .num_of_peers_for_bootstrap_threshold(),
        mempool_prevalidator_factory.clone(),
        identity.clone(),
        initialize_chain_manager_result_callback,
        tokio_runtime.handle(),
    )
    .expect("Failed to create chain manager");

    if let Err(e) = initialize_chain_manager_result_callback_receiver
        .recv_timeout(env.initialize_chain_manager_timeout)
    {
        panic!("Chain manager was not initialized within {:?} timeout, e.g. try increase [--initialize-chain-manager-timeout] and check logs for errors, reason: {}", env.initialize_chain_manager_timeout, e)
    };
    info!(log, "Chain manager initialized (8/8)");

    let shell_connector =
        ShellConnectorSupport::new(chain_manager.clone(), mempool_prevalidator_factory.clone());

    // initialize rpc server
    let mut rpc_server = RpcServer::new(
        log.clone(),
        Box::new(shell_connector),
        ([0, 0, 0, 0], env.rpc.listener_port).into(),
        tokio_runtime.handle().clone(),
        &persistent_storage,
        current_mempool_state_storage,
        Arc::clone(&tezos_protocol_api),
        env.tezos_network_config,
        Arc::new(shell_compatibility_version.to_network_version()),
        &init_storage_data,
        hydrated_current_head_block,
        env.storage
            .context_storage_configuration
            .tezedge_is_enabled(),
    );
    let _ = RpcNotificationCallbackActor::actor(
        actor_system.as_ref(),
        shell_channel.clone(),
        rpc_server.rpc_env(),
    )
    .expect("Failed to create rpc notification callback handler actor");

    // Only start Monitoring when websocket is set
    if let Some((websocket_address, max_number_of_websocket_connections)) = env.rpc.websocket_cfg {
        let websocket_handler = WebsocketHandler::actor(
            actor_system.as_ref(),
            tokio_runtime.handle().clone(),
            websocket_address,
            max_number_of_websocket_connections,
            log.clone(),
        )
        .expect("Failed to start websocket actor");

        let _ = Monitor::actor(
            actor_system.as_ref(),
            network_channel.clone(),
            websocket_handler,
            shell_channel.clone(),
            persistent_storage.clone(),
            init_storage_data.chain_id.clone(),
        )
        .expect("Failed to create monitor actor");
    }

    if let Some(blocks) = blocks_replay.take() {
        return schedule_replay_blocks(
            blocks,
            &init_storage_data,
            chain_manager,
            block_applier,
            block_applier_thread_watcher,
            shell_channel,
            log.clone(),
        );
    } else {
        // TODO: TE-386 - controlled startup
        std::thread::sleep(std::time::Duration::from_secs(2));

        // and than open p2p and others
        let _ = PeerManager::actor(
            actor_system.as_ref(),
            network_channel,
            shell_channel.clone(),
            tokio_runtime.handle().clone(),
            identity,
            shell_compatibility_version,
            env.p2p,
            env.identity.expected_pow,
        )
        .expect("Failed to create peer manager");
    }

    let mut is_setup_ok = true;

    // start rpc
    if let Err(e) = rpc_server.start() {
        error!(log, "Failed to start RPC server"; "reason" => format!("{:?}", e));
        is_setup_ok = false;
    };

    info!(log, "Actors initialized");

    tokio_runtime.block_on(async move {
        use tokio::signal;
        use tokio::time::timeout;

        // if everything is ok, we can run and hold this "forever"
        if is_setup_ok {
            signal::ctrl_c()
                .await
                .expect("Failed to listen for ctrl-c event");
            info!(log, "Ctrl-c or SIGINT received!");
        }

        info!(log, "Shutting down rpc server (1/8)");
        drop(rpc_server);

        info!(log, "Shutting down of thread workers starting (2/8)");
        if let Err(e) = block_applier_thread_watcher.stop() {
            warn!(log, "Failed to stop thread watcher";
                       "thread_name" => block_applier_thread_watcher.thread_name(),
                       "reason" => format!("{}", e));
        }
        if let Err(e) = chain_manager_p2p_reader_thread_watcher.stop() {
            warn!(log, "Failed to stop thread watcher";
                       "thread_name" => chain_manager_p2p_reader_thread_watcher.thread_name(),
                       "reason" => format!("{}", e));
        }
        let mempool_thread_watchers = match mempool_prevalidator_factory
            .mempool_thread_watchers()
            .lock()
        {
            Ok(mut mempool_prevalidator_factory) => {
                let (mempool_thread_watchers_stop_initiated, _): (
                    Vec<ThreadWatcher>,
                    Vec<ThreadWatcher>,
                ) = mempool_prevalidator_factory
                    .drain()
                    .map(|(_, b)| b)
                    .partition(|thread_worker| {
                        if let Err(e) = thread_worker.stop() {
                            warn!(log, "Failed to stop thread watcher";
                               "thread_name" => thread_worker.thread_name(),
                               "reason" => format!("{}", e));
                            false
                        } else {
                            true
                        }
                    });
                mempool_thread_watchers_stop_initiated
            }
            Err(e) => {
                warn!(log, "Failed to lock mempool thread watchers, so we cannot gracefully release them";
                           "reason" => format!("{}", e));
                vec![]
            }
        };
        info!(log, "Shutting down of thread workers initiated"; "mempool_thread_watchers_count" => mempool_thread_watchers.len());

        info!(log, "Sending shutdown notification to actors (3/8)");
        shell_channel.tell(
            Publish {
                msg: ShuttingDown.into(),
                topic: ShellChannelTopic::ShellShutdown.into(),
            },
            None,
        );

        // give actors some time to shut down
        info!(log, "Waiting 2s for shutdown of actors (2/6)");
        tokio::time::sleep(Duration::from_secs(2)).await;

        info!(log, "Shutting down actors (4/8)");
        match timeout(Duration::from_secs(10), actor_system.shutdown()).await {
            Ok(_) => info!(log, "Shutdown actors complete"),
            Err(_) => warn!(log, "Shutdown actors did not finish to timeout (10s)"),
        };

        info!(log, "Waiting for thread workers finish gracefully (please, wait, it could take some time) (5/8)");
        if let Some(thread) = block_applier_thread_watcher.thread() {
            thread.thread().unpark();
            if let Err(e) = thread.join() {
                warn!(log, "Failed to wait for block applier thread"; "reason" => format!("{:?}", e));
            }
        }
        if let Some(thread) = chain_manager_p2p_reader_thread_watcher.thread() {
            thread.thread().unpark();
            if let Err(e) = thread.join() {
                warn!(log, "Failed to wait for p2p reader thread"; "reason" => format!("{:?}", e));
            }
        }
        for mut mempool_thread_watcher in mempool_thread_watchers {
            if let Some(thread) = mempool_thread_watcher.thread() {
                thread.thread().unpark();
                if let Err(e) = thread.join() {
                    warn!(log, "Failed to wait for block applier thread"; "reason" => format!("{:?}", e));
                }
            }
        }
        info!(log, "Thread workers stopped");

        info!(log, "Shutting down protocol runner pools (6/8)");
        drop(tezos_protocol_api);
        debug!(log, "Protocol runners completed");

        info!(log, "Flushing databases (7/8)");
        drop(persistent_storage);
        info!(log, "Databases flushed");

        info!(log, "Shutdown complete (8/8)");
    });
}

fn check_deprecated_network(env: &Environment, log: &Logger) {
    if let Some(deprecation_notice) = env.tezos_network.check_deprecated_network() {
        warn!(log, "Deprecated network: {}", deprecation_notice);
    }
}

fn schedule_replay_blocks(
    blocks: Vec<Arc<BlockHash>>,
    init_storage_data: &StorageInitInfo,
    chain_manager: ChainManagerRef,
    block_applier: ChainFeederRef,
    block_applier_thread_watcher: ThreadWatcher,
    shell_channel: ShellChannelRef,
    log: Logger,
) {
    let chain_manager = Arc::new(chain_manager);
    let chain_id = Arc::new(init_storage_data.chain_id.clone());
    let (result_callback_sender, result_callback_receiver) = create_oneshot_callback();
    let fail_above = init_storage_data.replay.as_ref().unwrap().fail_above;
    let nblocks = blocks.len();
    let now = std::time::Instant::now();

    for (index, block) in blocks.into_iter().enumerate() {
        let batch = ApplyBlockBatch::one((&*block).clone());
        let result_callback = Some(result_callback_sender.clone());

        let now = std::time::Instant::now();
        block_applier.tell(
            ApplyBlock::new(
                Arc::clone(&chain_id),
                batch,
                chain_manager.clone(),
                result_callback,
                None,
                None,
            ),
            None,
        );

        let result = result_callback_receiver
            .recv()
            .expect("Failed to wait for block aplication");
        let time = now.elapsed();

        let hash = block.to_base58_check();
        let percent = (index as f64 / nblocks as f64) * 100.0;

        if result.as_ref().is_err() {
            replay_shutdown(&log, shell_channel, block_applier_thread_watcher);
            panic!(
                "{:08} {:.5}% Block {} failed in {:?}. Result={:?}",
                index, percent, hash, time, result
            );
        } else if time > fail_above && index > 0 {
            replay_shutdown(&log, shell_channel, block_applier_thread_watcher);
            panic!(
                "{:08} {:.5}% Block {} processed in {:?} (more than {:?}). Result={:?}",
                index, percent, hash, time, fail_above, result
            );
        } else {
            info!(
                log,
                "{:08} {:.5}% Block {} applied in {:?}. Result={:?}",
                index,
                percent,
                hash,
                time,
                result
            );
        }
    }

    info!(
        log,
        "Replayer successfully applied {} blocks in {:?}",
        nblocks,
        now.elapsed()
    );

    replay_shutdown(&log, shell_channel, block_applier_thread_watcher);
}

fn replay_shutdown(
    log: &Logger,
    shell_channel: ShellChannelRef,
    mut block_applier_thread_watcher: ThreadWatcher,
) {
    if let Err(e) = block_applier_thread_watcher.stop() {
        warn!(log, "Failed to stop thread watcher";
                       "thread_name" => block_applier_thread_watcher.thread_name(),
                       "reason" => format!("{}", e));
    }

    shell_channel.tell(
        Publish {
            msg: ShuttingDown.into(),
            topic: ShellChannelTopic::ShellShutdown.into(),
        },
        None,
    );

    // give actors some time to shut down
    std::thread::sleep(Duration::from_secs(2));

    if let Some(thread) = block_applier_thread_watcher.thread() {
        thread.thread().unpark();
        if let Err(e) = thread.join() {
            warn!(log, "Failed to wait for block applier thread"; "reason" => format!("{:?}", e));
        }
    }
}

fn collect_replayed_blocks(
    persistent_storage: &PersistentStorage,
    replay: &Replay,
    log: &Logger,
) -> Vec<Arc<BlockHash>> {
    let block_meta_storage = BlockMetaStorage::new(persistent_storage);
    let from_block = replay.from_block.as_ref();
    let mut block_hash = replay.to_block.clone();
    let mut blocks = Vec::new();
    let mut first_block_reached = false;

    info!(log, "Replayer is collecting blocks");

    if block_meta_storage
        .get(&block_hash)
        .map(|h| h.is_none())
        .unwrap_or(true)
    {
        panic!("Unable to find block {:?}", block_hash.to_base58_check());
    }

    while let Ok(Some(block_meta)) = block_meta_storage.get(&block_hash) {
        blocks.push(Arc::new(block_hash.clone()));

        first_block_reached = block_meta.level() == 1;
        if from_block == Some(&block_hash) || first_block_reached {
            break;
        }

        block_hash = match block_meta.predecessor() {
            Some(block) => block.clone(),
            None => {
                panic!(
                    "Unable to find predecessor of level={:?} block_hash={:?}",
                    block_meta.level(),
                    block_hash.to_base58_check()
                );
            }
        }
    }

    blocks.reverse();
    match (from_block, blocks.get(0)) {
        (Some(block), first) if blocks.is_empty() || block != &**first.unwrap() => {
            panic!("Unable to find block {:?}", block);
        }
        (None, _) if !first_block_reached => {
            panic!("Unable to find first block");
        }
        _ => {}
    }

    info!(log, "Replayer will apply {} blocks", blocks.len());

    blocks
}

#[cfg(dyncov)]
fn set_gcov_handler() {
    use signal_hook::{consts::SIGUSR2, iterator::Signals};
    let mut signals = Signals::new(&[SIGUSR2]).unwrap();

    extern "C" {
        fn __gcov_dump();
        fn __gcov_reset();
    }

    std::thread::spawn(move || {
        for sig in signals.forever() {
            eprintln!("!!! SIGUSR2: saving coverage info...");
            unsafe {
                __gcov_dump();
                __gcov_reset();
            }
        }
    });
}

fn main() {
    #[cfg(dyncov)]
    set_gcov_handler();

    // Parses config + cli args
    let env = crate::configuration::Environment::from_args();

    // Creates loggers
    let log = match env.create_logger() {
        Ok(log) => log,
        Err(e) => panic!(
            "Error while creating loggers, check '--log' argument, reason: {:?}",
            e
        ),
    };

    // Enable core dumps and increase open files limit
    system::init_limits(&log);

    // log configuration
    info!(
        log,
        "Loaded configuration";
        "cfg" => &env
    );

    // check deprecated networks
    info!(
        log,
        "Configured network {:?} -> {}",
        env.tezos_network.supported_values(),
        env.tezos_network_config.version
    );
    check_deprecated_network(&env, &log);

    // Validate zcash-params
    info!(log, "Checking zcash-params for sapling... (1/8)");
    if let Err(e) = env.ffi.zcash_param.assert_zcash_params(&log) {
        let description = env.ffi.zcash_param.description("'--init-sapling-spend-params-file=<spend-file-path>' / '--init-sapling-output-params-file=<output-file-path'");
        error!(log, "Failed to validate zcash-params required for sapling support"; "description" => description.clone(), "reason" => format!("{}", e));
        panic!(
            "Failed to validate zcash-params required for sapling support, reason: {}, description: {}",
            e, description
        );
    }

    // Loads tezos identity based on provided identity-file argument. In case it does not exist, it will try to automatically generate it
    info!(log, "Loading identity... (2/8)");
    let tezos_identity = match identity::ensure_identity(&env.identity, &log) {
        Ok(identity) => {
            info!(log, "Identity loaded from file";
                       "file" => env.identity.identity_json_file_path.as_path().display().to_string(),
                       "peer_id" => identity.peer_id.to_base58_check());
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

    // create/initialize databases
    info!(log, "Loading databases... (3/8)");

    // create common RocksDB block cache to be shared among column families
    // IMPORTANT: Cache object must live at least as long as DB (returned by open_kv)
    let mut caches = GlobalRocksDbCacheHolder::with_capacity(1);
    let main_chain = MainChain::new(
        env.tezos_network_config
            .main_chain_id()
            .expect("Failed to decode chainId"),
        env.tezos_network_config.version.clone(),
    );

    // initialize dbs
    let kv_cache = RocksDbCache::new_lru_cache(env.storage.db.cache_size)
        .expect("Failed to initialize RocksDB cache (db)");

    let maindb = match env.storage.main_db {
        TezedgeDatabaseBackendConfiguration::Sled => initialize_maindb(
            &log,
            None,
            &env.storage.db,
            env.storage.db.expected_db_version,
            &main_chain,
            env.storage.main_db,
        )
        .expect("Failed to create/initialize MainDB database (db)"),
        TezedgeDatabaseBackendConfiguration::RocksDB => {
            let kv = initialize_rocksdb(&log, &kv_cache, &env.storage.db, &main_chain)
                .expect("Failed to create/initialize RocksDB database (db)");
            caches.push(kv_cache);
            initialize_maindb(
                &log,
                Some(kv),
                &env.storage.db,
                env.storage.db.expected_db_version,
                &main_chain,
                env.storage.main_db,
            )
            .expect("Failed to create/initialize MainDB database (db)")
        }
    };

    let commit_logs = Arc::new(
        open_cl(
            &env.storage.db_path,
            vec![BlockStorage::descriptor()],
            log.clone(),
        )
        .expect("Failed to open plain block_header storage"),
    );
    let sequences = Arc::new(Sequences::new(maindb.clone(), 1000));

    {
        let persistent_storage = PersistentStorage::new(maindb, commit_logs, sequences);

        match resolve_storage_init_chain_data(
            &env.tezos_network_config,
            &env.storage.db_path,
            &env.storage.context_storage_configuration,
            &env.storage.patch_context,
            &env.storage.context_stats_db_path,
            &env.replay,
            &log,
        ) {
            Ok(init_data) => {
                let blocks_replay = env
                    .replay
                    .as_ref()
                    .map(|replay| collect_replayed_blocks(&persistent_storage, replay, &log));

                info!(log, "Databases loaded successfully");
                block_on_actors(
                    env,
                    init_data,
                    Arc::new(tezos_identity),
                    persistent_storage,
                    blocks_replay,
                    log,
                )
            }
            Err(e) => panic!("Failed to resolve init storage chain data, reason: {}", e),
        }
    }
}
