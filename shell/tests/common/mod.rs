// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use slog::{Drain, Level, Logger};

pub fn prepare_empty_dir(dir_name: &str) -> String {
    let path = test_storage_dir_path(dir_name);
    if path.exists() {
        fs::remove_dir_all(&path).unwrap_or_else(|_| panic!("Failed to delete directory: {:?}", &path));
    }
    fs::create_dir_all(&path).unwrap_or_else(|_| panic!("Failed to create directory: {:?}", &path));
    String::from(path.to_str().unwrap())
}

pub fn test_storage_dir_path(dir_name: &str) -> PathBuf {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR is not defined");
    let path = Path::new(out_dir.as_str())
        .join(Path::new(dir_name))
        .to_path_buf();
    path
}

pub fn create_logger(level: Level) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(
            slog_term::TermDecorator::new().build()
        ).build().fuse()
    ).build().filter_level(level).fuse();

    Logger::root(drain, slog::o!())
}

pub fn is_ocaml_log_enabled() -> bool {
    env::var("OCAML_LOG_ENABLED")
        .unwrap_or("false".to_string())
        .parse::<bool>().unwrap()
}

pub fn no_of_ffi_calls_treshold_for_gc() -> i32 {
    env::var("OCAML_CALLS_GC")
        .unwrap_or("2000".to_string())
        .parse::<i32>().unwrap()
}

pub fn log_level() -> Level {
    env::var("LOG_LEVEL")
        .unwrap_or("info".to_string())
        .parse::<Level>().unwrap()
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
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;
    use std::time::Duration;

    use riker::actors::*;
    use riker::system::SystemBuilder;
    use slog::{Logger, warn};
    use tokio::runtime::Runtime;

    use networking::p2p::network_channel::{NetworkChannel, NetworkChannelRef};
    use shell::chain_feeder::ChainFeeder;
    use shell::chain_manager::ChainManager;
    use shell::context_listener::ContextListener;
    use shell::mempool_prevalidator::MempoolPrevalidator;
    use shell::PeerConnectionThreshold;
    use shell::shell_channel::{ShellChannel, ShellChannelRef, ShellChannelTopic, ShuttingDown};
    use storage::resolve_storage_init_chain_data;
    use storage::tests_common::TmpStorage;
    use tezos_api::environment::{TEZOS_ENV, TezosEnvironmentConfiguration};
    use tezos_api::ffi::TezosRuntimeConfiguration;
    use tezos_wrapper::{TezosApiConnectionPool, TezosApiConnectionPoolConfiguration};
    use tezos_wrapper::service::{ExecutableProtocolRunner, ProtocolEndpointConfiguration, ProtocolRunnerEndpoint};

    use crate::{common, test_data};

    pub struct NodeInfrastructure {
        name: String,
        pub log: Logger,
        pub shell_channel: ShellChannelRef,
        pub network_channel: NetworkChannelRef,
        pub actor_system: ActorSystem,
        pub tmp_storage: TmpStorage,
        pub tezos_env: TezosEnvironmentConfiguration,
        pub tokio_runtime: Runtime,
        apply_restarting_feature: Arc<AtomicBool>,
    }

    impl NodeInfrastructure {
        pub fn start(test_storage_path: &str, name: &str) -> Result<Self, failure::Error> {
            // logger
            let log_level = common::log_level();
            let log = common::create_logger(log_level.clone());

            warn!(log, "Starting node infrastructure"; "name" => name);

            // environement
            let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV.get(&test_data::TEZOS_NETWORK).expect("no environment configuration");
            let is_sandbox = false;
            let p2p_threshold = PeerConnectionThreshold::new(1, 1);

            // storage
            let storage_db_path = test_storage_path;
            let context_db_path = common::prepare_empty_dir(&format!("{}_context", storage_db_path));
            let tmp_storage = TmpStorage::create(common::prepare_empty_dir(storage_db_path))?;
            let persistent_storage = tmp_storage.storage();

            let storage_db_path = PathBuf::from(storage_db_path);
            let context_db_path = PathBuf::from(context_db_path);
            let init_storage_data = resolve_storage_init_chain_data(&tezos_env, &storage_db_path, &context_db_path, &None, &log)
                .expect("Failed to resolve init storage chain data");

            // apply block protocol runner endpoint
            let apply_protocol_runner = common::protocol_runner_executable_path();
            let mut apply_protocol_runner_endpoint = ProtocolRunnerEndpoint::<ExecutableProtocolRunner>::new(
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
                    log_level.clone(),
                    true,
                ),
                log.clone(),
            );
            let (apply_restarting_feature, apply_protocol_commands, apply_protocol_events) = match apply_protocol_runner_endpoint.start_in_restarting_mode() {
                Ok(restarting_feature) => {
                    let ProtocolRunnerEndpoint {
                        commands,
                        events,
                        ..
                    } = apply_protocol_runner_endpoint;
                    (restarting_feature, commands, events)
                }
                Err(e) => panic!("Error to start test_protocol_runner_endpoint: {} - error: {:?}", apply_protocol_runner.as_os_str().to_str().unwrap_or("-none-"), e)
            };

            // create pool for ffi protocol runner connections (used just for readonly context)
            let tezos_readonly_api = Arc::new(
                TezosApiConnectionPool::new_with_readonly_context(
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
                )
            );

            let tokio_runtime = create_tokio_runtime();

            // run actor's
            let actor_system = SystemBuilder::new().name(name).log(log.clone()).create().expect("Failed to create actor system");
            let shell_channel = ShellChannel::actor(&actor_system).expect("Failed to create shell channel");
            let network_channel = NetworkChannel::actor(&actor_system).expect("Failed to create network channel");
            let _ = ContextListener::actor(&actor_system, &persistent_storage, apply_protocol_events.expect("Context listener needs event server"), log.clone(), false).expect("Failed to create context event listener");
            let _ = ChainFeeder::actor(&actor_system, shell_channel.clone(), &persistent_storage, &init_storage_data, &tezos_env, apply_protocol_commands, log.clone()).expect("Failed to create chain feeder");
            let _ = ChainManager::actor(&actor_system, network_channel.clone(), shell_channel.clone(), &persistent_storage, &init_storage_data.chain_id, is_sandbox, &p2p_threshold).expect("Failed to create chain manager");
            let _ = MempoolPrevalidator::actor(
                &actor_system,
                shell_channel.clone(),
                &persistent_storage,
                &init_storage_data,
                tezos_readonly_api.clone(),
                log.clone(),
            ).expect("Failed to create chain feeder");

            Ok(
                NodeInfrastructure {
                    name: String::from(name),
                    log,
                    apply_restarting_feature,
                    shell_channel,
                    network_channel,
                    tokio_runtime,
                    actor_system,
                    tmp_storage,
                    tezos_env: tezos_env.clone(),
                }
            )
        }

        pub fn stop(&mut self) {
            warn!(self.log, "Stopping node infrastructure"; "name" => self.name.clone());

            // clean up
            // shutdown events listening
            self.apply_restarting_feature.store(false, Ordering::Release);

            thread::sleep(Duration::from_secs(3));
            self.shell_channel.tell(
                Publish {
                    msg: ShuttingDown.into(),
                    topic: ShellChannelTopic::ShellCommands.into(),
                }, None,
            );
            thread::sleep(Duration::from_secs(2));

            let _ = self.actor_system.shutdown();
            warn!(self.log, "Node infrastructure stopped"; "name" => self.name.clone());
        }
    }

    fn create_tokio_runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new()
            .basic_scheduler()
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime")
    }
}