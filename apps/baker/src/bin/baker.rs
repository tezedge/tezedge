// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    fs::{self, File},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::SystemTime,
};

use reqwest::Url;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
pub struct Arguments {
    #[structopt(long)]
    base_dir: PathBuf,
    #[structopt(long)]
    baker: String,
    #[structopt(long)]
    endpoint: Url,
    #[structopt(short, long)]
    archive: bool,
    // #[structopt(long)]
    // node_dir: Option<PathBuf>,
}

fn main() {
    use baker::{machine::*, Services};
    use redux_rs::Store;

    let Arguments {
        base_dir,
        baker,
        endpoint,
        archive,
    } = Arguments::from_args();

    let env = env_logger::Env::default().default_filter_or("info");
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .try_init()
        .unwrap();

    let terminating = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&terminating))
        .expect("cannot handle signals");

    let (srv, events) = Services::new(endpoint, &base_dir, &baker);
    let chain_id = srv.client.get_chain_id().unwrap();
    let _ = srv.client.wait_bootstrapped().unwrap();
    let constants = srv.client.get_constants().unwrap();
    srv.client.monitor_heads(&chain_id).unwrap();
    let log = srv.log.clone();

    // store the state here, and then atomically swap to avoid corruption
    // due to unexpected power outage
    let file_path_swap = base_dir.join(format!(".state_{baker}_{chain_id}.json"));

    let archive_path = base_dir.join(format!("state_{baker}_{chain_id}_archive"));
    if archive {
        let _ = fs::create_dir_all(&archive_path);
    }

    let file_path = base_dir.join(format!("state_{baker}_{chain_id}.json"));
    let persistent_state = File::open(&file_path)
        .and_then(|rdr| serde_json::from_reader::<_, BakerState>(rdr).map_err(From::from));

    let mut initial_state = if let Ok(persistent_state) = persistent_state {
        persistent_state
    } else {
        BakerState::new(chain_id, constants, srv.crypto.public_key_hash().clone())
    };

    if let Ok(mut prev_seeds) = File::open(base_dir.join("seed_patch.json")).and_then(|rdr| serde_json::from_reader::<_, std::collections::BTreeMap<baker::machine::Nonce, u32>>(rdr).map_err(From::from)) {
        initial_state.as_mut().nonces.previous.append(&mut prev_seeds);
    }

    let initial_state = BakerStateEjectable(Some(initial_state));
    let reducer = baker_reducer::<BakerStateEjectable, Action>;
    let effects = baker_effects::<BakerStateEjectable, Services, Action>;
    let initial_time = SystemTime::now();
    let mut store = Store::new(reducer, effects, srv, initial_time, initial_state);
    let mut previous_checkpoint = (0, 0);
    for event in events {
        store.dispatch::<BakerAction>(event.action.into());
        let state = store.state.get().as_ref().as_ref().unwrap();
        let st = state.as_ref();
        let this_checkpoint = (
            st.tb_state.level().unwrap_or(0),
            st.tb_state.round().unwrap_or(0),
        );
        if this_checkpoint != previous_checkpoint {
            let file_swap = File::create(&file_path_swap).expect("msg");
            serde_json::to_writer(file_swap, state).unwrap();
            fs::rename(&file_path_swap, &file_path).unwrap();
            let _ = fs::remove_file(&file_path_swap);
            slog::info!(log, "stored on disk");

            if archive {
                let name = format!("_{}_{}.json", this_checkpoint.0, this_checkpoint.1);
                let dst = archive_path.join(name);
                slog::info!(log, "archive {}", dst.display());
                fs::copy(&file_path, dst).unwrap();
            }

            if terminating.load(Ordering::SeqCst) {
                // ctrl+c pressed, state is on disk, terminate the baker
                slog::info!(log, "terminated gracefully");
                break;
            }
        }
        previous_checkpoint = this_checkpoint;
    }
}
