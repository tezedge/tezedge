// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
use self::command_line::{Arguments, Command};

mod logger;

mod client;
use self::client::TezosClient;

mod types;

mod key;

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

    // std::process::Command::new("cp").arg("-R").arg(&base_dir).arg("/home/vscode/workspace/tezedge/target/d").output().unwrap();

    let log = logger::logger(false, slog::Level::Info);
    let requests_logger = if log_requests {
        log.clone()
    } else {
        logger::logger(true, slog::Level::Info)
    };
    let main_logger = logger::main_logger();
    let (client, _) = TezosClient::new(requests_logger, main_logger.clone(), endpoint);

    let service = ServiceDefault {
        log,
        main_logger,
        client,
    };
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
}
