// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::SystemTime;

fn main() {
    use baker::{machine::*, Arguments, Command, Services};
    use redux_rs::Store;

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
            let chain_id = srv.client.get_chain_id().unwrap();
            let _ = srv.client.wait_bootstrapped().unwrap();
            let constants = srv.client.get_constants().unwrap();
            srv.client.monitor_heads(&chain_id).unwrap();

            let initial_state =
                BakerState::new(chain_id, constants, srv.crypto.public_key_hash().clone());
            let initial_state = BakerStateEjectable(Some(initial_state));
            let reducer = baker_reducer::<BakerStateEjectable, Action>;
            let effects = baker_effects::<BakerStateEjectable, Services, Action>;
            let initial_time = SystemTime::now();
            let mut store = Store::new(reducer, effects, srv, initial_time, initial_state);
            for event in events {
                store.dispatch::<BakerAction>(event.event.into());
            }
        }
    }
}
