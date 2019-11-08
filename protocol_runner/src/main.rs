// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::thread;

use clap::{App, Arg};
use slog::*;

use tezos_context::channel;

fn create_logger() -> Logger {
    let drain = slog_async::Async::new(slog_term::FullFormat::new(slog_term::TermDecorator::new().build()).build().fuse()).build().filter_level(Level::Info).fuse();

    Logger::root(drain, slog::o!())
}

fn main() {
    let log = create_logger();

    let matches = App::new("Protocol Runner")
        .version("1.0")
        .author("Tomas Sedlak <tomas.sedlak@simplestaking.com>")
        .about("Tezos Protocol Runner")
        .arg(Arg::with_name("sock")
            .short("s")
            .long("sock")
            .value_name("SOCK")
            .help("Path to communication socket")
            .takes_value(true)
            .empty_values(false)
            .required(true))
        .get_matches();

    let socket_path = matches.value_of("sock").expect("Missing sock value");

    ctrlc::set_handler(move || {
        // do nothing and wait for parent process to send termination command
    }).expect("Error setting Ctrl-C handler");

    channel::enable_context_channel();
    thread::spawn(|| {
        while let Ok(action) = channel::context_receive() {

        }
    });

    let res = tezos_wrapper::service::process_protocol_messages::<crate::tezos::NativeTezosLib, _>(socket_path);
    if res.is_err() {
        error!(log, "Error while processing protocol messages"; "reason" => format!("{:?}", res.unwrap_err()));
    }
}

mod tezos {
    use tezos_api::client::TezosStorageInitInfo;
    use tezos_api::environment::TezosEnvironment;
    use tezos_api::ffi::{ApplyBlockError, ApplyBlockResult, TezosRuntimeConfiguration, TezosRuntimeConfigurationError, TezosStorageInitError};
    use tezos_client::client::{apply_block, change_runtime_configuration, init_storage};
    use tezos_encoding::hash::{BlockHash, ChainId};
    use tezos_messages::p2p::encoding::prelude::*;
    use tezos_wrapper::protocol::ProtocolApi;

    pub struct NativeTezosLib;

    impl ProtocolApi for NativeTezosLib {
        fn apply_block(chain_id: &ChainId, block_header_hash: &BlockHash, block_header: &BlockHeader, operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ApplyBlockError> {
            apply_block(chain_id, block_header_hash, block_header, operations)
        }

        fn change_runtime_configuration(settings: TezosRuntimeConfiguration) -> Result<(), TezosRuntimeConfigurationError> {
            change_runtime_configuration(settings)
        }

        fn init_storage(storage_data_dir: String, tezos_environment: TezosEnvironment) -> Result<TezosStorageInitInfo, TezosStorageInitError> {
            init_storage(storage_data_dir, tezos_environment)
        }
    }
}