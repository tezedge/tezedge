// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

/// Simple integration test for actors
///
/// (Tests are ignored, because they need protocol-runner binary)
/// Runs like: `PROTOCOL_RUNNER=./target/release/protocol-runner cargo test --release -- --ignored`
use std::sync::atomic::Ordering;
use std::time::Duration;

use lazy_static::lazy_static;
use serial_test::serial;

use networking::ShellCompatibilityVersion;
use shell::peer_manager::P2p;
use shell::PeerConnectionThreshold;
use storage::tests_common::TmpStorage;
use tezos_api::environment::{TezosEnvironmentConfiguration, TEZOS_ENV};
use tezos_identity::Identity;

pub mod common;

lazy_static! {
    pub static ref SHELL_COMPATIBILITY_VERSION: ShellCompatibilityVersion = ShellCompatibilityVersion::new("TEST_CHAIN".to_string(), vec![0], vec![0]);
    pub static ref NODE_P2P_PORT: u16 = 1234; // TODO: maybe some logic to verify and get free port
    pub static ref NODE_P2P_CFG: (P2p, ShellCompatibilityVersion) = (
        P2p {
            listener_port: *NODE_P2P_PORT,
            bootstrap_lookup_addresses: vec![],
            disable_bootstrap_lookup: true,
            disable_mempool: false,
            private_node: false,
            bootstrap_peers: vec![],
            peer_threshold: PeerConnectionThreshold::try_new(0, 2, Some(0)).expect("Invalid range"),
        },
        SHELL_COMPATIBILITY_VERSION.clone(),
    );
    pub static ref NODE_IDENTITY: Identity = tezos_identity::Identity::generate(0f64).unwrap();
}

#[ignore]
#[test]
#[serial]
fn test_peer_threshold() -> Result<(), failure::Error> {
    // logger
    let log_level = common::log_level();
    let log = common::create_logger(log_level);

    // prepare env data
    let (tezos_env, patch_context) = {
        let (db, patch_context) = common::test_cases_data::sandbox_branch_1_level3::init_data(&log);
        (db.tezos_env, patch_context)
    };
    let tezos_env: &TezosEnvironmentConfiguration = TEZOS_ENV
        .get(&tezos_env)
        .expect("no environment configuration");

    // start node
    let node = common::infra::NodeInfrastructure::start(
        TmpStorage::create(common::prepare_empty_dir("__test_22"))?,
        &common::prepare_empty_dir("__test_22_context"),
        "test_peer_threshold",
        &tezos_env,
        patch_context,
        Some(NODE_P2P_CFG.clone()),
        NODE_IDENTITY.clone(),
        (log, log_level),
    )?;

    // wait for storage initialization to genesis
    node.wait_for_new_current_head(
        "genesis",
        node.tezos_env.genesis_header_hash()?,
        (Duration::from_secs(5), Duration::from_millis(250)),
    )?;

    let mut incoming_peers: Vec<common::test_node_peer::TestNodePeer> = Vec::new();
    for peer_index in 0..4 {
        let name: &'static str =
            Box::leak(format!("{}{}", "TEST_PEER_", peer_index).into_boxed_str());
        incoming_peers.push(common::test_node_peer::TestNodePeer::connect(
            name,
            NODE_P2P_CFG.0.listener_port,
            NODE_P2P_CFG.1.clone(),
            tezos_identity::Identity::generate(0f64)?,
            node.log.clone(),
            &node.tokio_runtime,
            common::test_cases_data::sandbox_branch_1_level3::serve_data,
        ));
    }
    // TODO (Hotfix): TE- Remove this sleep after the p2p rework
    // Note: Now we are essentially connecting after the threshold, but immediatly disconnecting so we need a delay here
    std::thread::sleep(Duration::from_secs(3));
    assert_eq!(
        incoming_peers
            .iter()
            .map(|peer| peer.connected.load(Ordering::Acquire))
            .filter(|con| con == &true)
            .count(),
        2
    );

    Ok(())
}
