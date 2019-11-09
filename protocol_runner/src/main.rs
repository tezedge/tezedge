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
        .arg(Arg::with_name("sock-cmd")
            .short("c")
            .long("sock-cmd")
            .value_name("path")
            .help("Path to a command socket")
            .takes_value(true)
            .empty_values(false)
            .required(true))
        .arg(Arg::with_name("sock-evt")
            .short("e")
            .long("sock-evt")
            .value_name("path")
            .help("Path to a event socket")
            .takes_value(true)
            .empty_values(false)
            .required(true))
        .get_matches();

    let cmd_socket_path = matches.value_of("sock-cmd").expect("Missing sock-cmd value");
    let evt_socket_path = matches.value_of("sock-evt").expect("Missing sock-evt value").to_string();

    ctrlc::set_handler(move || {
        // do nothing and wait for parent process to send termination command
    }).expect("Error setting Ctrl-C handler");

    channel::enable_context_channel();
    let event_thread = thread::spawn(move || {
        tezos_wrapper::service::process_protocol_events(&evt_socket_path)
    });

    let res = tezos_wrapper::service::process_protocol_commands::<crate::tezos::NativeTezosLib, _>(cmd_socket_path);
    if res.is_err() {
        error!(log, "Error while processing protocol commands"; "reason" => format!("{:?}", res.unwrap_err()));
    }

    let res = event_thread.join().expect("Couldn't join on the associated thread");
    if res.is_err() {
        error!(log, "Error while processing protocol events"; "reason" => format!("{:?}", res.unwrap_err()));
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