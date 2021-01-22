// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use slog::{Drain, Level, Logger};

use crypto::hash::OperationHash;
use tezos_messages::p2p::encoding::prelude::Operation;

pub fn prepare_empty_dir(dir_name: &str) -> String {
    let path = test_storage_dir_path(dir_name);
    if path.exists() {
        fs::remove_dir_all(&path)
            .unwrap_or_else(|_| panic!("Failed to delete directory: {:?}", &path));
    }
    fs::create_dir_all(&path).unwrap_or_else(|_| panic!("Failed to create directory: {:?}", &path));
    String::from(path.to_str().unwrap())
}

pub fn test_storage_dir_path(dir_name: &str) -> PathBuf {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    Path::new(out_dir.as_str()).join(Path::new(dir_name))
}

pub fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .build()
    .filter_level(level)
    .fuse();

    Logger::root(drain, slog::o!())
}

pub fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or_else(|_| "false".to_string())
        .parse::<bool>()
        .unwrap()
}

pub fn no_of_ffi_calls_treshold_for_gc() -> i32 {
    env::var("OCAML_CALLS_GC")
        .unwrap_or_else(|_| "2000".to_string())
        .parse::<i32>()
        .unwrap()
}

pub fn log_level() -> Level {
    env::var("LOG_LEVEL")
        .unwrap_or_else(|_| "info".to_string())
        .parse::<Level>()
        .unwrap()
}

pub fn protocol_runner_executable_path() -> PathBuf {
    let executable = env::var("PROTOCOL_RUNNER")
        .unwrap_or_else(|_| panic!("This test requires environment parameter: 'PROTOCOL_RUNNER' to point to protocol_runner executable"));
    PathBuf::from(executable)
}

/// Empty message
#[derive(Serialize, Deserialize, Debug)]
pub struct NoopMessage;

/// Module which runs actor's very similar than real node runs
#[allow(dead_code)]
pub mod infra {
    use std::collections::HashSet;
    use std::path::PathBuf;
    use std::process::Child;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::thread::JoinHandle;
    use std::time::{Duration, SystemTime};

    use riker::actors::*;
    use riker::system::SystemBuilder;
    use slog::{info, warn, Level, Logger};
    use tokio::runtime::Runtime;

    use crypto::hash::{BlockHash, ContextHash, HashType, OperationHash};
    use networking::p2p::network_channel::{NetworkChannel, NetworkChannelRef};
    use networking::ShellCompatibilityVersion;
    use shell::chain_feeder::ChainFeeder;
    use shell::chain_manager::ChainManager;
    use shell::context_listener::ContextListener;
    use shell::mempool::mempool_prevalidator::MempoolPrevalidator;
    use shell::mempool::{init_mempool_state_storage, CurrentMempoolStateStorageRef};
    use shell::peer_manager::{P2p, PeerManager, PeerManagerRef, WhitelistAllIpAddresses};
    use shell::shell_channel::{ShellChannel, ShellChannelRef, ShellChannelTopic, ShuttingDown};
    use shell::PeerConnectionThreshold;
    use storage::chain_meta_storage::ChainMetaStorageReader;
    use storage::context::{ContextApi, TezedgeContext};
    use storage::tests_common::TmpStorage;
    use storage::{resolve_storage_init_chain_data, BlockStorage, ChainMetaStorage};
    use tezos_api::environment::TezosEnvironmentConfiguration;
    use tezos_api::ffi::{PatchContext, TezosRuntimeConfiguration};
    use tezos_identity::Identity;
    use tezos_wrapper::runner::{ExecutableProtocolRunner, ProtocolRunner};
    use tezos_wrapper::service::ProtocolRunnerEndpoint;
    use tezos_wrapper::ProtocolEndpointConfiguration;
    use tezos_wrapper::{TezosApiConnectionPool, TezosApiConnectionPoolConfiguration};

    use crate::common;
    use crate::common::contains_all_keys;

    pub struct NodeInfrastructure {
        name: String,
        pub log: Logger,
        pub peer_manager: Option<PeerManagerRef>,
        pub shell_channel: ShellChannelRef,
        pub network_channel: NetworkChannelRef,
        pub actor_system: ActorSystem,
        pub tmp_storage: TmpStorage,
        pub current_mempool_state_storage: CurrentMempoolStateStorageRef,
        pub tezos_env: TezosEnvironmentConfiguration,
        pub tokio_runtime: Runtime,

        apply_block_restarting_feature: Arc<AtomicBool>,
        apply_block_restarting_thread: Arc<Mutex<Option<JoinHandle<Child>>>>,
    }

    impl NodeInfrastructure {
        pub fn start(
            tmp_storage: TmpStorage,
            context_db_path: &str,
            name: &str,
            tezos_env: &TezosEnvironmentConfiguration,
            patch_context: Option<PatchContext>,
            p2p: Option<(P2p, ShellCompatibilityVersion)>,
            identity: Identity,
            (log, log_level): (Logger, Level),
        ) -> Result<Self, failure::Error> {
            warn!(log, "[NODE] Starting node infrastructure"; "name" => name);

            // environement
            let is_sandbox = false;
            let p2p_threshold = PeerConnectionThreshold::new(1, 1, Some(0));
            let identity = Arc::new(identity);

            // storage
            let persistent_storage = tmp_storage.storage();
            let context_db_path = if !PathBuf::from(context_db_path).exists() {
                common::prepare_empty_dir(context_db_path)
            } else {
                context_db_path.to_string()
            };
            let current_mempool_state_storage = init_mempool_state_storage();

            let context_db_path = PathBuf::from(context_db_path);
            let init_storage_data = resolve_storage_init_chain_data(
                &tezos_env,
                &tmp_storage.path(),
                &context_db_path,
                &patch_context,
                &log,
            )
            .expect("Failed to resolve init storage chain data");

            // apply block protocol runner endpoint
            let apply_protocol_runner = common::protocol_runner_executable_path();
            let mut apply_protocol_runner_endpoint = ProtocolRunnerEndpoint::<
                ExecutableProtocolRunner,
            >::try_new(
                &format!("{}_write_runner", name),
                ProtocolEndpointConfiguration::new(
                    TezosRuntimeConfiguration {
                        log_enabled: common::is_ocaml_log_enabled(),
                        no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
                        debug_mode: false,
                    },
                    tezos_env.clone(),
                    false,
                    &context_db_path,
                    &apply_protocol_runner,
                    log_level,
                    true,
                ),
                log.clone(),
            )?;
            let (apply_restarting_feature, apply_protocol_commands, apply_protocol_events) =
                match apply_protocol_runner_endpoint.start_in_restarting_mode() {
                    Ok(restarting_feature) => {
                        let ProtocolRunnerEndpoint {
                            commands, events, ..
                        } = apply_protocol_runner_endpoint;
                        (restarting_feature, commands, events)
                    }
                    Err(e) => panic!(
                        "Error to start test_protocol_runner_endpoint: {} - error: {:?}",
                        apply_protocol_runner
                            .as_os_str()
                            .to_str()
                            .unwrap_or("-none-"),
                        e
                    ),
                };

            // create pool for ffi protocol runner connections (used just for readonly context)
            let tezos_readonly_api = Arc::new(TezosApiConnectionPool::new_with_readonly_context(
                String::from(&format!("{}_readonly_runner_pool", name)),
                TezosApiConnectionPoolConfiguration {
                    min_connections: 0,
                    max_connections: 2,
                    connection_timeout: Duration::from_secs(3),
                    max_lifetime: Duration::from_secs(60),
                    idle_timeout: Duration::from_secs(60),
                },
                ProtocolEndpointConfiguration::new(
                    TezosRuntimeConfiguration {
                        log_enabled: common::is_ocaml_log_enabled(),
                        no_of_ffi_calls_treshold_for_gc: common::no_of_ffi_calls_treshold_for_gc(),
                        debug_mode: false,
                    },
                    tezos_env.clone(),
                    false,
                    &context_db_path,
                    &common::protocol_runner_executable_path(),
                    log_level,
                    false,
                ),
                log.clone(),
            ));

            let tokio_runtime = create_tokio_runtime();

            // run actor's
            let actor_system = SystemBuilder::new()
                .name(name)
                .log(log.clone())
                .create()
                .expect("Failed to create actor system");
            let shell_channel =
                ShellChannel::actor(&actor_system).expect("Failed to create shell channel");
            let network_channel =
                NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
            let _ = ContextListener::actor(
                &actor_system,
                &persistent_storage,
                apply_protocol_events.expect("Context listener needs event server"),
                log.clone(),
                false,
            )
            .expect("Failed to create context event listener");
            let block_applier = ChainFeeder::actor(
                &actor_system,
                shell_channel.clone(),
                &persistent_storage,
                &init_storage_data,
                &tezos_env,
                apply_protocol_commands,
                log.clone(),
            )
            .expect("Failed to create chain feeder");
            let _ = ChainManager::actor(
                &actor_system,
                block_applier,
                network_channel.clone(),
                shell_channel.clone(),
                persistent_storage.clone(),
                tezos_readonly_api.clone(),
                init_storage_data.chain_id.clone(),
                is_sandbox,
                current_mempool_state_storage.clone(),
                false,
                &p2p_threshold,
                identity.clone(),
            )
            .expect("Failed to create chain manager");
            let _ = MempoolPrevalidator::actor(
                &actor_system,
                shell_channel.clone(),
                &persistent_storage,
                current_mempool_state_storage.clone(),
                init_storage_data.chain_id,
                tezos_readonly_api,
                log.clone(),
            )
            .expect("Failed to create chain feeder");

            // and than open p2p and others - if configured
            let peer_manager = if let Some((p2p_config, shell_compatibility_version)) = p2p {
                let peer_manager = PeerManager::actor(
                    &actor_system,
                    network_channel.clone(),
                    shell_channel.clone(),
                    tokio_runtime.handle().clone(),
                    identity,
                    Arc::new(shell_compatibility_version),
                    p2p_config,
                )
                .expect("Failed to create peer manager");
                Some(peer_manager)
            } else {
                None
            };

            Ok(NodeInfrastructure {
                name: String::from(name),
                log,
                apply_block_restarting_feature: apply_restarting_feature.0,
                apply_block_restarting_thread: Arc::new(Mutex::new(Some(
                    apply_restarting_feature.1,
                ))),
                peer_manager,
                shell_channel,
                network_channel,
                tokio_runtime,
                actor_system,
                tmp_storage,
                current_mempool_state_storage,
                tezos_env: tezos_env.clone(),
            })
        }

        fn stop(&mut self) {
            let NodeInfrastructure {
                log,
                apply_block_restarting_feature,
                apply_block_restarting_thread,
                shell_channel,
                actor_system,
                tokio_runtime,
                ..
            } = self;
            warn!(log, "[NODE] Stopping node infrastructure"; "name" => self.name.clone());

            // clean up
            // shutdown events listening
            // disable/stop protocol runner for applying blocks feature
            // let (apply_blocks_protocol_runner_endpoint_run_feature, apply_blocks_protocol_runner_endpoint_watchdog_thread) = apply_restarting_feature;
            // stop restarting feature
            apply_block_restarting_feature.store(false, Ordering::Release);

            shell_channel.tell(
                Publish {
                    msg: ShuttingDown.into(),
                    topic: ShellChannelTopic::ShellShutdown.into(),
                },
                None,
            );

            let apply_restarting_feature = apply_block_restarting_thread
                .lock()
                .expect("Failed to lock mutex")
                .take();

            if let Some(apply_restarting_feature) = apply_restarting_feature {
                if let Ok(mut protocol_runner_process) = apply_restarting_feature.join() {
                    if let Err(e) = ExecutableProtocolRunner::wait_and_terminate_ref(
                        &mut protocol_runner_process,
                        Duration::from_secs(2),
                    ) {
                        warn!(log, "Failed to terminate/kill protocol runner"; "reason" => e);
                    }
                };
            }

            let _ = tokio_runtime.block_on(actor_system.shutdown());

            warn!(log, "[NODE] Node infrastructure stopped"; "name" => self.name.clone());
        }

        // TODO: refactor with async/condvar, not to block main thread
        pub fn wait_for_new_current_head(
            &self,
            marker: &str,
            tested_head: BlockHash,
            (timeout, delay): (Duration, Duration),
        ) -> Result<(), failure::Error> {
            let start = SystemTime::now();
            let tested_head = Some(tested_head).map(|th| HashType::BlockHash.hash_to_b58check(&th));

            let chain_meta_data = ChainMetaStorage::new(self.tmp_storage.storage());
            let result = loop {
                let current_head = chain_meta_data
                    .get_current_head(&self.tezos_env.main_chain_id()?)?
                    .map(|ch| {
                        let ch: BlockHash = ch.into();
                        ch
                    })
                    .map(|ch| HashType::BlockHash.hash_to_b58check(&ch));

                if current_head.eq(&tested_head) {
                    info!(self.log, "[NODE] Expected current head detected"; "head" => tested_head, "marker" => marker);
                    break Ok(());
                }

                // kind of simple retry policy
                if start.elapsed()?.le(&timeout) {
                    thread::sleep(delay);
                } else {
                    break Err(failure::format_err!("wait_for_new_current_head({:?}) - timeout (timeout: {:?}, delay: {:?}) exceeded! marker: {}", tested_head, timeout, delay, marker));
                }
            };
            result
        }

        // TODO: refactor with async/condvar, not to block main thread
        /// Context_listener is now asynchronous, so we need to make sure, that it is processed, so we wait a little bit
        pub fn wait_for_context(
            &self,
            marker: &str,
            context_hash: ContextHash,
            (timeout, delay): (Duration, Duration),
        ) -> Result<(), failure::Error> {
            let start = SystemTime::now();

            let context = TezedgeContext::new(
                BlockStorage::new(self.tmp_storage.storage()),
                self.tmp_storage.storage().merkle(),
            );

            // try checkout context
            let result = loop {
                // if success, than ok
                if let Ok(true) = context.is_committed(&context_hash) {
                    info!(self.log, "[NODE] Expected context found"; "context_hash" => HashType::ContextHash.hash_to_b58check(&context_hash), "marker" => marker);
                    break Ok(());
                }

                // kind of simple retry policy
                if start.elapsed()?.le(&timeout) {
                    thread::sleep(delay);
                } else {
                    break Err(failure::format_err!("wait_for_context({:?}) - timeout (timeout: {:?}, delay: {:?}) exceeded! marker: {}", HashType::ContextHash.hash_to_b58check(&context_hash), timeout, delay, marker));
                }
            };
            result
        }

        // TODO: refactor with async/condvar, not to block main thread
        pub fn wait_for_mempool_on_head(
            &self,
            marker: &str,
            tested_head: BlockHash,
            (timeout, delay): (Duration, Duration),
        ) -> Result<(), failure::Error> {
            let start = SystemTime::now();
            let tested_head = Some(tested_head).map(|th| HashType::BlockHash.hash_to_b58check(&th));

            let result = loop {
                let mempool_state = self
                    .current_mempool_state_storage
                    .read()
                    .expect("Failed to obtain lock");
                let current_head = mempool_state
                    .head()
                    .map(|ch| HashType::BlockHash.hash_to_b58check(&ch));

                if current_head.eq(&tested_head) {
                    info!(self.log, "[NODE] Expected mempool head detected"; "head" => tested_head, "marker" => marker);
                    break Ok(());
                }

                // kind of simple retry policy
                if start.elapsed()?.le(&timeout) {
                    thread::sleep(delay);
                } else {
                    break Err(failure::format_err!("wait_for_mempool_on_head({:?}) - timeout (timeout: {:?}, delay: {:?}) exceeded! marker: {}", tested_head, timeout, delay, marker));
                }
            };
            result
        }

        // TODO: refactor with async/condvar, not to block main thread
        pub fn wait_for_mempool_contains_operations(
            &self,
            marker: &str,
            expected_operations: &HashSet<OperationHash>,
            (timeout, delay): (Duration, Duration),
        ) -> Result<(), failure::Error> {
            let start = SystemTime::now();

            let result = loop {
                let mempool_state = self
                    .current_mempool_state_storage
                    .read()
                    .expect("Failed to obtain lock");
                if contains_all_keys(mempool_state.operations(), expected_operations) {
                    info!(self.log, "[NODE] All expected operations found in mempool"; "marker" => marker);
                    break Ok(());
                }
                drop(mempool_state);

                // kind of simple retry policy
                if start.elapsed()?.le(&timeout) {
                    thread::sleep(delay);
                } else {
                    break Err(failure::format_err!("wait_for_mempool_contains_operations() - timeout (timeout: {:?}, delay: {:?}) exceeded! marker: {}", timeout, delay, marker));
                }
            };
            result
        }

        pub fn whitelist_all(&self) {
            if let Some(peer_manager) = &self.peer_manager {
                peer_manager.tell(WhitelistAllIpAddresses, None);
            }
        }
    }

    impl Drop for NodeInfrastructure {
        fn drop(&mut self) {
            warn!(self.log, "[NODE] Dropping node infrastructure"; "name" => self.name.clone());
            self.stop();
        }
    }

    fn create_tokio_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    }
}

fn contains_all_keys(
    map: &HashMap<OperationHash, Operation>,
    keys: &HashSet<OperationHash>,
) -> bool {
    let mut contains_counter = 0;
    for key in keys {
        if map.contains_key(key) {
            contains_counter += 1;
        }
    }
    contains_counter == keys.len()
}
