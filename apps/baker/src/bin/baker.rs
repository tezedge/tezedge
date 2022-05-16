// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs::{self, File},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::SystemTime,
};

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
            let log = srv.log.clone();

            let terminating = Arc::new(AtomicBool::new(false));
            signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminating))
                .expect("cannot handle signals");

            // store the state here, and then atomically swap to avoid corruption
            // due to unexpected power outage
            let file_path_swap = base_dir.join(format!("state_{baker}_{chain_id}.swap.json"));

            let file_path = base_dir.join(format!("state_{baker}_{chain_id}.json"));
            let persistent_state = File::open(&file_path)
                .and_then(|rdr| serde_json::from_reader::<_, BakerState>(rdr).map_err(From::from));

            let initial_state = if let Ok(persistent_state) = persistent_state {
                persistent_state
            } else {
                BakerState::new(chain_id, constants, srv.crypto.public_key_hash().clone())
            };

            let initial_state = BakerStateEjectable(Some(initial_state));
            let reducer = baker_reducer::<BakerStateEjectable, Action>;
            let effects = baker_effects::<BakerStateEjectable, Services, Action>;
            let initial_time = SystemTime::now();
            let mut store = Store::new(reducer, effects, srv, initial_time, initial_state);
            let mut previous_is_idle = true;
            for event in events {
                store.dispatch::<BakerAction>(event.action.into());
                let state = store.state.get().as_ref().as_ref().unwrap();
                if let BakerState::Idle(_) = state {
                    if !previous_is_idle {
                        // the state is idle, store the state into file
                        let _ = fs::remove_file(&file_path_swap);
                        let file_swap = File::create(&file_path_swap).expect("msg");
                        serde_json::to_writer(file_swap, state).unwrap();
                        fs::rename(&file_path_swap, &file_path).unwrap();
                        slog::info!(log, "stored on disk");

                        if terminating.load(Ordering::SeqCst) {
                            // ctrl+c pressed, state is on disk, terminate the baker
                            slog::info!(log, "terminated gracefully");
                            break;
                        }
                    }
                    previous_is_idle = true;
                } else {
                    previous_is_idle = false;
                }
            }
        }
    }
}
