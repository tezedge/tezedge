// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
mod key;
mod logger;
mod proof_of_work;
mod seed_nonce;

mod alternative;

fn main() {
    use self::{
        command_line::{Arguments, Command},
        key::CryptoService,
    };

    let Arguments {
        base_dir,
        endpoint,
        log_requests: _,
        command,
    } = Arguments::from_args();

    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .try_init()
        .unwrap();

    let logger = logger::main_logger();

    match command {
        Command::RunWithLocalNode { node_dir, baker } => {
            // We don't use context storage and protocol_runner
            let _ = node_dir;

            let crypto = match CryptoService::read_key(&base_dir, &baker) {
                Ok(v) => v,
                Err(err) => {
                    slog::error!(logger, "error creating crypto service: {err}");
                    return;
                }
            };
            slog::info!(logger, "crypto service ready: {}", crypto.public_key_hash());

            alternative::run(endpoint.clone(), &crypto, &logger, &base_dir, &baker).unwrap();
        }
    }
}
