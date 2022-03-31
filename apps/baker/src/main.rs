// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
mod proof_of_work;

mod alternative;
mod services;
mod machine;

fn main() {
    use self::command_line::{Arguments, Command};
    use self::services::Services;

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

    match command {
        Command::RunWithLocalNode { node_dir, baker } => {
            // We don't use context storage and protocol_runner
            let _ = node_dir;

            let (srv, events) = Services::new(endpoint, &base_dir, &baker);
            slog::info!(srv.log, "crypto service ready: {}", srv.crypto.public_key_hash());
            
            alternative::run(srv, &base_dir, &baker, events).unwrap();
        }
    }
}
