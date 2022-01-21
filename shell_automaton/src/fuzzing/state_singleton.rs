use once_cell::sync::Lazy;
use std::{convert::TryInto, env, io::Read, sync::RwLock};
use storage::persistent::Decoder;

use crate::State;

pub static FUZZER_STATE: Lazy<RwLock<State>> = Lazy::new(|| RwLock::new(initial_state()));

fn initial_state() -> State {
    let url = env::var("STATE_SNAPSHOT_URL").unwrap_or(String::from(
        "http://127.0.0.1:18732/dev/shell/automaton/state_raw",
    ));

    println!("Fetching state from: {}", url);

    let resp = match ureq::get(&url).call() {
        Ok(resp) => resp,
        Err(err) => {
            println!("Fetching state failed {:?}", err);
            std::process::exit(1);
        }
    };

    assert!(resp.has("Content-Length"));
    let len: usize = resp.header("Content-Length").unwrap().parse().unwrap();

    println!("Current state size: {} bytes", len);
    let mut bytes: Vec<u8> = Vec::with_capacity(len);

    resp.into_reader()
        .take(len.try_into().unwrap())
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len(), len);

    State::decode(&bytes).unwrap()
}
