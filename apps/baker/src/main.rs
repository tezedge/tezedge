// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
use self::command_line::{Arguments, Command};

mod logger;

mod client;
use self::client::{TezosClient, TezosClientEvent};

mod machine;
use self::machine::{action::*, effects, reducer, service::ServiceDefault, state::State};

fn main() {
    use std::time::SystemTime;

    let Arguments {
        base_dir,
        endpoint,
        log_requests,
        command,
    } = Arguments::from_args();

    let log = logger::logger(false, slog::Level::Info);
    let requests_logger = if log_requests {
        log.clone()
    } else {
        logger::logger(true, slog::Level::Info)
    };
    let (client, rx) = TezosClient::new(requests_logger, endpoint);

    let service = ServiceDefault { log, client };
    let initial_time = SystemTime::now();
    let initial_state = State::default();

    let mut store = redux_rs::Store::new(reducer, effects, service, initial_time, initial_state);
    match command {
        Command::RunWithLocalNode { node_dir, baker } => {
            store.dispatch(RunWithLocalNodeAction {
                base_dir,
                node_dir,
                baker,
            });
        }
    }

    while let Ok(event) = rx.recv() {
        match event {
            TezosClientEvent::Bootstrapped => {
                store.dispatch(BootstrappedAction);
            }
            TezosClientEvent::NewHead(head) => {
                store.dispatch(NewHeadSeenAction { head });
            }
            TezosClientEvent::Operation(operation) => {
                store.dispatch(NewOperationSeenAction { operation });
            }
        }
    }
}
