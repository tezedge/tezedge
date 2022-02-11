// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
mod logger;
mod machine;
mod rpc_client;
mod types;
mod key;

fn main() {
    use std::time::SystemTime;
    use self::{
        command_line::{Arguments, Command},
        rpc_client::RpcClient,
        key::CryptoService,
        machine::{action::*, effects, reducer, service::ServiceDefault, state::State},
    };

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

            let crypto = CryptoService::read_key(&base_dir, &baker).unwrap();

            let service = ServiceDefault {
                logger,
                client,
                crypto,
            };
            let initial_time = SystemTime::now();
            let initial_state = State::Initial;

            let mut store = redux_rs::Store::new(reducer, effects, service, initial_time, initial_state);
            store.dispatch(GetChainIdInitAction {});
            for event in events {
                store.dispatch(event);
            }
        }
    }
}
