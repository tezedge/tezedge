// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

//! Separate Tezos protocol runner, as we used OCaml protocol more and more, we noticed increasing
//! problems, from panics to high memory usage, for better stability, we separated protocol into
//! self-contained process communicating through Unix Socket.

mod ipc_loop;

use clap::{App, Arg};
use slog::*;

extern crate jemallocator;

#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

fn create_logger(log_level: Level, endpoint_name: String) -> Logger {
    let drain = slog_async::Async::new(
        slog_term::FullFormat::new(slog_term::TermDecorator::new().build())
            .build()
            .fuse(),
    )
    .build()
    .filter_level(log_level)
    .fuse();

    Logger::root(drain, slog::o!("endpoint" => endpoint_name))
}

fn main() {
    let matches = App::new("TezEdge Protocol Runner")
        .version(env!("CARGO_PKG_VERSION"))
        .author("TezEdge and the project contributors")
        .about("Tezos Protocol Runner")
        .arg(
            Arg::with_name("sock-cmd")
                .short("c")
                .long("sock-cmd")
                .value_name("path")
                .help("Path to a command socket")
                .takes_value(true)
                .empty_values(false)
                .required(true),
        )
        .arg(
            Arg::with_name("endpoint")
                .long("endpoint")
                .value_name("STRING")
                .help("Name of the endpoint, which spawned runner")
                .takes_value(true)
                .empty_values(false)
                .required(true),
        )
        .arg(
            Arg::with_name("log-level")
                .long("log-level")
                .takes_value(true)
                .value_name("LEVEL")
                .possible_values(&["critical", "error", "warn", "info", "debug", "trace"])
                .help("Set log level"),
        )
        .get_matches();

    let cmd_socket_path = matches
        .value_of("sock-cmd")
        .expect("Missing sock-cmd value");
    let endpoint_name = matches
        .value_of("endpoint")
        .expect("Missing endpoint value")
        .to_string();
    let log_level = matches
        .value_of("log-level")
        .unwrap_or("info")
        .parse::<slog::Level>()
        .expect("Was expecting one value from slog::Level");

    let log = create_logger(log_level, endpoint_name);

    let shutdown_callback = |log: &Logger| {
        debug!(log, "Shutting down OCaml runtime");
        match std::panic::catch_unwind(|| {
            tezos_client::client::shutdown_runtime();
        }) {
            Ok(_) => debug!(log, "OCaml runtime shutdown was successful"),
            Err(e) => {
                warn!(log, "Shutting down OCaml runtime failed (check running sub-process for this endpoint or `[protocol-runner] <defunct>`, and and terminate/kill manually)!"; "reason" => format!("{:?}", e))
            }
        }
    };

    {
        let log = log.clone();
        // do nothing and wait for parent process to send termination command
        // this is just fallback, if ProtocolController.shutdown will fail or if we need to kill sub-process manually
        ctrlc::set_handler(move || {
            shutdown_callback(&log);
            warn!(log, "Protocol runner was terminated/killed/ctrl-c - please, check running sub-processes for `[protocol-runner] <defunct>`, and terminate/kill manually!");
        }).expect("Error setting Ctrl-C handler");
    }

    // Process commands from from the Rust node. Most commands are instructions for the Tezos protocol
    if let Err(err) = ipc_loop::process_protocol_commands::<crate::tezos::NativeTezosLib, _, _>(
        cmd_socket_path,
        &log,
        shutdown_callback,
    ) {
        error!(log, "Error while processing protocol commands"; "reason" => format!("{:?}", err));
        shutdown_callback(&log);
    }

    info!(log, "Protocol runner finished gracefully");
}

mod tezos {
    use crypto::hash::{ChainId, ContextHash, ProtocolHash};
    use tezos_api::ffi::{
        ApplyBlockError, ApplyBlockRequest, ApplyBlockResponse, BeginApplicationError,
        BeginApplicationRequest, BeginApplicationResponse, BeginConstructionError,
        BeginConstructionRequest, CommitGenesisResult, ComputePathError, ComputePathRequest,
        ComputePathResponse, FfiJsonEncoderError, GetDataError, HelpersPreapplyBlockRequest,
        HelpersPreapplyError, HelpersPreapplyResponse, InitProtocolContextResult,
        PrevalidatorWrapper, ProtocolDataError, ProtocolRpcError, ProtocolRpcRequest,
        ProtocolRpcResponse, RustBytes, TezosRuntimeConfiguration, TezosRuntimeConfigurationError,
        TezosStorageInitError, ValidateOperationError, ValidateOperationRequest,
        ValidateOperationResponse,
    };
    use tezos_client::client::*;
    use tezos_context_api::TezosContextConfiguration;
    use tezos_messages::p2p::encoding::operation::Operation;
    use tezos_wrapper::protocol::ProtocolApi;

    pub struct NativeTezosLib;

    impl ProtocolApi for NativeTezosLib {
        fn apply_block(request: ApplyBlockRequest) -> Result<ApplyBlockResponse, ApplyBlockError> {
            apply_block(request)
        }

        fn begin_application(
            request: BeginApplicationRequest,
        ) -> Result<BeginApplicationResponse, BeginApplicationError> {
            begin_application(request)
        }

        fn begin_construction(
            request: BeginConstructionRequest,
        ) -> Result<PrevalidatorWrapper, BeginConstructionError> {
            begin_construction(request)
        }

        fn validate_operation(
            request: ValidateOperationRequest,
        ) -> Result<ValidateOperationResponse, ValidateOperationError> {
            validate_operation(request)
        }

        fn call_protocol_rpc(
            request: ProtocolRpcRequest,
        ) -> Result<ProtocolRpcResponse, ProtocolRpcError> {
            call_protocol_rpc(request)
        }

        fn helpers_preapply_operations(
            request: ProtocolRpcRequest,
        ) -> Result<HelpersPreapplyResponse, HelpersPreapplyError> {
            helpers_preapply_operations(request)
        }

        fn helpers_preapply_block(
            request: HelpersPreapplyBlockRequest,
        ) -> Result<HelpersPreapplyResponse, HelpersPreapplyError> {
            helpers_preapply_block(request)
        }

        fn compute_path(
            request: ComputePathRequest,
        ) -> Result<ComputePathResponse, ComputePathError> {
            compute_path(request)
        }

        fn change_runtime_configuration(
            settings: TezosRuntimeConfiguration,
        ) -> Result<(), TezosRuntimeConfigurationError> {
            change_runtime_configuration(settings)
        }

        fn init_protocol_context(
            context_config: TezosContextConfiguration,
        ) -> Result<InitProtocolContextResult, TezosStorageInitError> {
            init_protocol_context(context_config)
        }

        fn genesis_result_data(
            genesis_context_hash: &ContextHash,
            chain_id: &ChainId,
            genesis_protocol_hash: &ProtocolHash,
            genesis_max_operations_ttl: u16,
        ) -> Result<CommitGenesisResult, GetDataError> {
            genesis_result_data(
                genesis_context_hash,
                chain_id,
                genesis_protocol_hash,
                genesis_max_operations_ttl,
            )
        }

        fn assert_encoding_for_protocol_data(
            protocol_hash: ProtocolHash,
            protocol_data: Vec<u8>,
        ) -> Result<(), ProtocolDataError> {
            assert_encoding_for_protocol_data(protocol_hash, protocol_data)
        }

        fn apply_block_result_metadata(
            context_hash: ContextHash,
            metadata_bytes: RustBytes,
            max_operations_ttl: i32,
            protocol_hash: ProtocolHash,
            next_protocol_hash: ProtocolHash,
        ) -> Result<String, FfiJsonEncoderError> {
            apply_block_result_metadata(
                context_hash,
                metadata_bytes,
                max_operations_ttl,
                protocol_hash,
                next_protocol_hash,
            )
        }

        fn apply_block_operations_metadata(
            chain_id: ChainId,
            operations: Vec<Vec<Operation>>,
            operations_metadata_bytes: Vec<Vec<RustBytes>>,
            protocol_hash: ProtocolHash,
            next_protocol_hash: ProtocolHash,
        ) -> Result<String, FfiJsonEncoderError> {
            apply_block_operations_metadata(
                chain_id,
                operations,
                operations_metadata_bytes,
                protocol_hash,
                next_protocol_hash,
            )
        }
    }
}
