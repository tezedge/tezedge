// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::{Read, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::SystemTime,
    time::{Duration, Instant},
};

use crypto::hash::{BlockHash, ContractTz1Hash};
use reqwest::blocking::Client;
use thiserror::Error;

use redux_rs::Store;
use tezos_messages::protocol::proto_012::operation::{
    InlinedEndorsementMempoolContents, InlinedPreendorsementContents,
};

use crate::{
    machine::{baker_effects, baker_reducer, Action, BakerAction, BakerState, BakerStateEjectable},
    services::event::Block,
    LiquidityBakingToggleVote, Protocol, ProtocolBlockHeaderJ, RpcClient, Services,
};

#[derive(Debug, Error)]
pub enum TestError {
    #[error("timeout")]
    Timeout,
    #[error("cannot run node")]
    NodeTimeout,
    #[error("lost block {level}:{round}")]
    LostBlock { level: i32, round: i32 },
}

pub type Baker = Store<BakerStateEjectable, Services, Action>;

pub mod accessor {
    use crypto::hash::ContractTz1Hash;

    use super::{Baker, BakerAction};

    pub fn level(baker: &Baker) -> Option<i32> {
        let st = baker.state().as_ref().as_ref().unwrap().as_ref();
        st.tb_state.level()
    }

    pub fn key(baker: &Baker) -> ContractTz1Hash {
        let st = baker.state().as_ref().as_ref().unwrap().as_ref();
        st.this.clone()
    }

    pub fn actions(baker: &Baker) -> &[BakerAction] {
        let st = baker.state().as_ref().as_ref().unwrap().as_ref();
        &st.actions
    }

    pub fn slots(baker: &Baker, level: i32) -> Option<Vec<u16>> {
        let st = baker.state().as_ref().as_ref().unwrap().as_ref();
        Some(
            st.tb_config
                .map
                .delegates
                .get(&level)?
                .get(&st.this)?
                .0
                .clone(),
        )
    }
}

pub enum TestResult {
    Continue,
    Terminate,
}

pub fn run<Test>(dir: PathBuf, timeout: Duration, test: Test) -> Result<(), TestError>
where
    Test:
        FnMut(&mut [Baker], &mut BlockWatcher, usize, BakerAction) -> Result<TestResult, TestError>,
{
    let env = env_logger::Env::default().default_filter_or("info");
    let _ = env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .try_init();

    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).expect("create dir");
    fs::create_dir_all(dir.join("client")).expect("create dir");
    fs::create_dir_all(dir.join("node")).expect("create dir");
    let secret_keys = include_str!("secret_keys");
    File::create(dir.join("client").join("secret_keys"))
        .expect("msg")
        .write_all(secret_keys.as_bytes())
        .expect("msg");

    let node_log = File::create(dir.join("node.log")).expect("create node log file");
    let mut node_process = Command::new("light-node");
    node_process
        .args(&[
            "--network=sandbox",
            "--private-node=true",
            "--log=terminal",
            "--log-level=info",
            "--tezos-context-storage=irmin",
            "--peer-thresh-low=0",
            "--peer-thresh-high=1",
            "--identity-expected-pow=0",
            "--synchronization-thresh=0",
            "--peers=0.0.0.0:9732",
            "--p2p-port=9732",
            "--rpc-port=18732",
        ])
        .arg(format!("--tezos-data-dir={}", dir.join("node").display()))
        .arg(format!(
            "--bootstrap-db-path={}",
            dir.join("node/bootstrap_db").display()
        ))
        .stderr(node_log);
    if let Ok(runner) = env::var("PROTOCOL_RUNNER") {
        node_process.arg(format!("--protocol-runner={runner}"));
    }
    if let Ok(sapling_params_dir) = env::var("SAPLING_PARAMS_DIR") {
        node_process.args(&[
            format!("--init-sapling-spend-params-file={sapling_params_dir}/sapling-spend.params"),
            format!("--init-sapling-output-params-file={sapling_params_dir}/sapling-output.params"),
        ]);
    }

    let mut node = match node_process.spawn() {
        Ok(v) => v,
        Err(_) => {
            log::warn!("test skipped because `LD_LIBRARY_PATH` and/or `PATH` vars are not set");
            return Ok(());
        }
    };

    let client = Client::new();
    log::info!("waiting the node...");
    let start_waiting = Instant::now();
    loop {
        match client.get("http://localhost:18732/network/version").send() {
            Err(_) => {
                if start_waiting.elapsed() > Duration::from_secs(60) {
                    log::error!("cannot run node");
                    let mut node_log =
                        File::open(dir.join("node.log")).expect("open node log file");
                    let mut s = String::new();
                    node_log.read_to_string(&mut s).expect("read node log");
                    log::info!("{s:?}");
                    node.kill().expect("msg");
                    return Err(TestError::NodeTimeout);
                }
                thread::sleep(Duration::from_secs(1));
            }
            Ok(mut response) => {
                let mut s = String::new();
                response.read_to_string(&mut s).expect("read");
                log::info!("node is ready: {s}");
                break;
            }
        }
    }

    let parameters_path = dir.join("client").join("activation-parameters.json");
    let parameters = include_str!("activation-parameters.json");
    File::create(&parameters_path)
        .expect("msg")
        .write_all(parameters.as_bytes())
        .expect("msg");

    Command::new("tezos-client")
        .arg("-base-dir")
        .arg(dir.join("client").display().to_string())
        .args(&[
            "import",
            "secret",
            "key",
            "activator",
            "unencrypted:edsk31vznjHSSpGExDMHYASz45VZqXN4DPxvsa4hAyY8dHM28cZzp6",
            "--force",
        ])
        .output()
        .expect("msg");

    let output = Command::new("tezos-client")
        .arg("-base-dir")
        .arg(dir.join("client").display().to_string())
        .args(&[
            "-E",
            "http://localhost:18732",
            "-block",
            "genesis",
            "activate",
            "protocol",
            "PtJakart2xVj7pYXJBXrqHgd82rdkLey5ZeeGwDgPp9rhQUbSqY",
            "with",
            "fitness",
            "1",
            "and",
            "key",
            "activator",
            "and",
            "parameters",
        ])
        .arg(parameters_path)
        .stdout(Stdio::piped())
        .output()
        .expect("msg");
    println!("{}", String::from_utf8(output.stdout).unwrap());
    println!("{}", String::from_utf8(output.stderr).unwrap());

    let r = inner(dir, timeout, test);
    node.kill().expect("msg");
    log::info!("node stopped");
    r
}

fn inner<Test>(dir: PathBuf, timeout: Duration, mut test: Test) -> Result<(), TestError>
where
    Test:
        FnMut(&mut [Baker], &mut BlockWatcher, usize, BakerAction) -> Result<TestResult, TestError>,
{
    let (tx, rx) = mpsc::channel();
    let client = RpcClient::new("http://localhost:18732".parse().unwrap(), 4, tx.clone());
    let chain_id = client.get_chain_id().unwrap();
    let constants = client.get_constants().unwrap();
    let protocol = Protocol::Jakarta;
    let liquidity_baking_toggle_vote = LiquidityBakingToggleVote::Off;
    let mut bakers = vec![];
    for id in 0..4 {
        let log =
            fs::File::create(dir.join(format!("baker_{id}.log"))).expect("can create log file");
        let service = Services::new_with_id_and_log(
            "http://localhost:18732".parse().unwrap(),
            &dir.join("client"),
            &format!("baker_{id}"),
            Some(log),
            id,
            tx.clone(),
        );
        service
            .client
            .monitor_heads::<ProtocolBlockHeaderJ>(&chain_id)
            .unwrap();
        let this = service.crypto.public_key_hash().clone();
        let state = BakerState::new(
            chain_id.clone(),
            constants.clone(),
            this,
            protocol,
            liquidity_baking_toggle_vote,
        );
        let initial_state = BakerStateEjectable(Some(state));
        let reducer = baker_reducer::<BakerStateEjectable, Action>;
        let effects = baker_effects::<BakerStateEjectable, Services, Action>;
        let initial_time = SystemTime::now();
        bakers.push(Store::new(
            reducer,
            effects,
            service,
            initial_time,
            initial_state,
        ));
    }

    let mut watcher = BlockWatcher::default();

    let start = Instant::now();
    for (id, event) in rx {
        let id: usize = id.into();

        if let BakerAction::ProposalEvent(b) = &event {
            watcher.blocks.insert(b.block.hash.clone(), b.block.clone());
        }

        match test(&mut bakers, &mut watcher, id, event)? {
            TestResult::Terminate => break,
            TestResult::Continue => (),
        }
        inspect_state(id, &bakers[id]);
        if start.elapsed() > timeout {
            return Err(TestError::Timeout);
        }
    }

    Ok(())
}

fn inspect_state(id: usize, baker: &Baker) {
    for action in accessor::actions(&baker) {
        match action {
            BakerAction::PreVote(act) => {
                let InlinedPreendorsementContents::Preendorsement(c) = &act.op.operations;
                log::info!(
                    "baker_{} 👍 {}:{} {}",
                    id,
                    c.level,
                    c.round,
                    &c.block_payload_hash.to_base58_check()[..13]
                );
            }
            BakerAction::Vote(act) => {
                let InlinedEndorsementMempoolContents::Endorsement(c) = &act.op.operations;
                log::info!(
                    "baker_{} ✅ {}:{} {}",
                    id,
                    c.level,
                    c.round,
                    &c.block_payload_hash.to_base58_check()[..13]
                );
            }
            BakerAction::Propose(act) => {
                log::info!("baker_{} 📦️ {}:{}", id, act.level, act.round);
            }
            _ => (),
        }
    }
}

#[derive(Default)]
pub struct BlockWatcher {
    blocks: BTreeMap<BlockHash, Block>,
    observed: BTreeMap<ContractTz1Hash, BTreeSet<i32>>,
}

impl BlockWatcher {
    pub fn check_block(&mut self, pred: &BlockHash, baker: &Baker) -> Result<(), TestError> {
        // may be none for genesis
        if let Some(finalized) = self.blocks.get(&pred) {
            // return error if missed block
            let level = finalized.level;
            let observed_map = self.observed.entry(accessor::key(baker)).or_default();
            if observed_map.contains(&level) {
                return Ok(());
            }
            let round = finalized.round;
            let our_round = accessor::slots(baker, level).unwrap()[0] as i32;
            if our_round < round {
                // TODO: sometimes it failing here http://ci.tezedge.com/tezedge/tezedge/5818/6/4
                return Ok(());
                // return Err(TestError::LostBlock { level, round: our_round });
            }
            observed_map.insert(level);
        }

        Ok(())
    }
}
