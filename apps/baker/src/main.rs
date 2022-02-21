// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::mpsc;

mod command_line;
mod key;
mod logger;
mod machine;
mod rpc_client;
mod timer;
mod types;

fn main() {
    use self::{
        command_line::{Arguments, Command},
        key::CryptoService,
        machine::{action::*, effects, reducer, service::ServiceDefault, state::State},
        rpc_client::RpcClient,
        timer::Timer,
    };
    use std::time::SystemTime;

    let Arguments {
        base_dir,
        endpoint,
        log_requests: _,
        command,
    } = Arguments::from_args();

    let logger = logger::main_logger();
    let (sender, events) = mpsc::channel();
    let client = RpcClient::new(endpoint, sender.clone());
    let timer = Timer::spawn(sender);

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
                timer,
            };
            let initial_time = SystemTime::now();
            let initial_state = State::Initial;

            slog::info!(logger, "creating state machine");
            let mut store =
                redux_rs::Store::new(reducer, effects, service, initial_time, initial_state);
            store.dispatch(GetChainIdInitAction {});
            for event in events.into_iter() {
                store.dispatch(event);
            }

            let service = store.service;
            service.timer.join().unwrap();
        }
    }
}
