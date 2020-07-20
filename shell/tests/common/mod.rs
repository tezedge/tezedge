// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{env, thread};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

use serde::{Deserialize, Serialize};
use slog::{Drain, Level, Logger};

use tezos_context::channel::enable_context_channel;
use tezos_wrapper::service::{ProtocolEndpointConfiguration, ProtocolRunner, ProtocolServiceError};

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

/// Control protocol runner sub-process.
#[derive(Clone)]
pub struct TestProtocolRunner {
    sock_cmd_path: PathBuf,
    sock_evt_path: Option<PathBuf>,
    endpoint_name: String,
}

impl ProtocolRunner for TestProtocolRunner {
    type Subprocess = Child;

    fn new(_configuration: ProtocolEndpointConfiguration, sock_cmd_path: &Path, sock_evt_path: Option<PathBuf>, endpoint_name: String) -> Self {
        TestProtocolRunner {
            sock_cmd_path: sock_cmd_path.to_path_buf(),
            sock_evt_path,
            endpoint_name,
        }
    }

    fn spawn(&self) -> Result<Child, ProtocolServiceError> {

        // run context_event callback listener
        let _ = if let Some(sock_evt_path) = &self.sock_evt_path {
            Some(
                {
                    let sock_evt_path = sock_evt_path.clone();
                    let endpoint_name = self.endpoint_name.clone();
                    // enable context event to receive
                    enable_context_channel();
                    thread::spawn(move || {
                        match tezos_wrapper::service::process_protocol_events(&sock_evt_path) {
                            Ok(()) => (),
                            Err(err) => {
                                println!("Error while processing protocol events, endpoint: {}. Reason: {}", &endpoint_name, format!("{:?}", err))
                            }
                        }
                    })
                }
            )
        } else {
            None
        };

        // Process commands from from the Rust node. Most commands are instructions for the Tezos protocol
        let _ = {
            let sock_cmd_path = self.sock_cmd_path.clone();
            let endpoint_name = self.endpoint_name.clone();
            thread::spawn(move || {
                if let Err(err) = tezos_wrapper::service::process_protocol_commands::<tezos::NativeTezosLib, _>(&sock_cmd_path) {
                    println!("Error while processing protocol commands, endpoint: {}. Reason: {}", &endpoint_name, format!("{:?}", err));
                }
            })
        };

        let process = Command::new("echo")
            .arg("...test protocol runner started!")
            .spawn()
            .map_err(|err| ProtocolServiceError::SpawnError { reason: err })?;
        Ok(process)
    }

    fn terminate(mut process: Child) {
        let _ = process.kill();
    }

    fn terminate_ref(process: &mut Child) {
        let _ = process.kill();
    }

    fn is_running(process: &mut Child) -> bool {
        match process.try_wait() {
            Ok(None) => true,
            _ => false,
        }
    }
}

mod tezos {
    use crypto::hash::{ChainId, ContextHash, ProtocolHash};
    use tezos_api::ffi::{ApplyBlockError, ApplyBlockResponse, BeginConstructionError, CommitGenesisResult, GenesisChain, GetDataError, InitProtocolContextResult, PatchContext, PrevalidatorWrapper, ProtocolOverrides, TezosGenerateIdentityError, TezosRuntimeConfiguration, TezosRuntimeConfigurationError, TezosStorageInitError, ValidateOperationError, ValidateOperationResponse};
    use tezos_api::identity::Identity;
    use tezos_client::client::{apply_block, begin_construction, change_runtime_configuration, generate_identity, genesis_result_data, init_protocol_context, validate_operation};
    use tezos_messages::p2p::encoding::prelude::*;
    use tezos_wrapper::protocol::ProtocolApi;

    pub struct NativeTezosLib;

    impl ProtocolApi for NativeTezosLib {
        fn apply_block(chain_id: &ChainId, block_header: &BlockHeader, predecessor_block_header: &BlockHeader, operations: &Vec<Option<OperationsForBlocksMessage>>, max_operations_ttl: u16) -> Result<ApplyBlockResponse, ApplyBlockError> {
            apply_block(chain_id, block_header, predecessor_block_header, operations, max_operations_ttl)
        }

        fn begin_construction(chain_id: &ChainId, block_header: &BlockHeader) -> Result<PrevalidatorWrapper, BeginConstructionError> {
            begin_construction(chain_id, block_header, None)
        }

        fn validate_operation(prevalidator: &PrevalidatorWrapper, operation: &Operation) -> Result<ValidateOperationResponse, ValidateOperationError> {
            validate_operation(prevalidator, operation)
        }

        fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), TezosRuntimeConfigurationError> {
            change_runtime_configuration(settings)
        }

        fn init_protocol_context(
            storage_data_dir: String,
            genesis: GenesisChain,
            protocol_overrides: ProtocolOverrides,
            commit_genesis: bool,
            enable_testchain: bool,
            readonly: bool,
            _: Option<PatchContext>) -> Result<InitProtocolContextResult, TezosStorageInitError> {
            init_protocol_context(storage_data_dir, genesis, protocol_overrides, commit_genesis, enable_testchain, readonly, None)
        }

        fn genesis_result_data(
            genesis_context_hash: &ContextHash,
            chain_id: &ChainId,
            genesis_protocol_hash: &ProtocolHash,
            genesis_max_operations_ttl: u16) -> Result<CommitGenesisResult, GetDataError> {
            genesis_result_data(genesis_context_hash, chain_id, genesis_protocol_hash, genesis_max_operations_ttl)
        }

        fn generate_identity(expected_pow: f64) -> Result<Identity, TezosGenerateIdentityError> {
            generate_identity(expected_pow)
        }
    }
}