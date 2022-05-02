// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod command_line;
mod proof_of_work;

// mod alternative;
mod machine;
mod services;

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

            let (mut srv, events) = Services::new(endpoint, &base_dir, &baker);
            let chain_id = srv.client.get_chain_id().unwrap();
            let _ = srv.client.wait_bootstrapped().unwrap();
            let constants = srv.client.get_constants().unwrap();
            srv.client.monitor_heads(&chain_id).unwrap();

            let mut state =
                machine::BakerState::new(chain_id, constants, srv.crypto.public_key_hash().clone());
            let mut actions = vec![];
            for event in events {
                state = state.handle_event(event, &mut actions);
                actions.drain(..).for_each(|action| srv.execute(action));
            }
        }
    }
}
