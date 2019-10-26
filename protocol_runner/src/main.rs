// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use clap::{App, Arg};
use slog::*;

fn create_logger() -> Logger {
    let drain = slog_async::Async::new(slog_term::FullFormat::new(slog_term::TermDecorator::new().build()).build().fuse()).build().filter_level(Level::Info).fuse();

    Logger::root(drain, slog::o!())
}

#[tokio::main]
async fn main() {
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

    let res = protocol_wrapper::service::process_protocol_messages::<crate::tezos::NativeTezosLib, _>(socket_path).await;
    if res.is_err() {
        error!(log, "Error while processing protocol messages"; "reason" => format!("{:?}", res.unwrap_err()));
    }
}

mod tezos {
    use protocol_wrapper::api::ProtocolApi;
    use tezos_client::client::{apply_block, change_runtime_configuration, init_storage, TezosStorageInitInfo};
    use tezos_client::environment::TezosEnvironment;
    use tezos_encoding::hash::{BlockHash, ChainId};
    use tezos_interop::ffi::{ApplyBlockError, ApplyBlockResult, OcamlRuntimeConfiguration, OcamlRuntimeConfigurationError, OcamlStorageInitError};
    use tezos_messages::p2p::encoding::prelude::*;

    pub struct NativeTezosLib;

    impl ProtocolApi for NativeTezosLib {
        fn apply_block(chain_id: &ChainId, block_header_hash: &BlockHash, block_header: &BlockHeader, operations: &Vec<Option<OperationsForBlocksMessage>>) -> Result<ApplyBlockResult, ApplyBlockError> {
            apply_block(chain_id, block_header_hash, block_header, operations)
        }

        fn change_runtime_configuration(settings: OcamlRuntimeConfiguration) -> Result<(), OcamlRuntimeConfigurationError> {
            change_runtime_configuration(settings)
        }

        fn init_storage(storage_data_dir: String, tezos_environment: TezosEnvironment) -> Result<TezosStorageInitInfo, OcamlStorageInitError> {
            init_storage(storage_data_dir, tezos_environment)
        }
    }
}