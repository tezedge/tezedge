// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
mod key;
mod logger;
mod machine;
mod rpc_client;
mod types;

fn main() {
    use self::{
        command_line::{Arguments, Command},
        key::CryptoService,
        machine::{action::*, effects, reducer, service::ServiceDefault, state::State},
        rpc_client::RpcClient,
    };
    use std::time::SystemTime;

    let Arguments {
        base_dir,
        endpoint,
        log_requests: _,
        command,
    } = Arguments::from_args();

    let logger = logger::main_logger();
    let (client, events) = RpcClient::new(endpoint);

    match command {
        Command::RunWithLocalNode { node_dir, baker } => {
            // We don't use context storage and protocol_runner
            let _ = node_dir;

            slog::info!(logger, "creating crypto service");
            let crypto = match CryptoService::read_key(&base_dir, &baker) {
                Ok(v) => v,
                Err(err) => {
                    slog::error!(logger, "error creating crypto service: {err}");
                    return;
                }
            };

            let service = ServiceDefault {
                logger: logger.clone(),
                client,
                crypto,
            };
            let initial_time = SystemTime::now();
            let initial_state = State::Initial;

            slog::info!(logger, "creating state machine");
            let mut store =
                redux_rs::Store::new(reducer, effects, service, initial_time, initial_state);
            store.dispatch(GetChainIdInitAction {});
            for event in events {
                store.dispatch(event);
            }
        }
    }
}
