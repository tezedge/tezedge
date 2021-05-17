// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![feature(test)]
extern crate test;

/// Simple integration test for p2p actors
///
/// (Tests are ignored, because they need protocol-runner binary)
/// Runs like: `PROTOCOL_RUNNER=./target/release/protocol-runner cargo test --release -- --ignored`
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
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
            listener_address: format!("0.0.0.0:{}", *NODE_P2P_PORT).parse::<SocketAddr>().expect("Failed to parse listener address"),
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

    // max peer's 4, means max incoming connections 2
    let peer_threshold_high = 4;

    // start node
    let node = common::infra::NodeInfrastructure::start(
        TmpStorage::create(common::prepare_empty_dir("__test_22"))?,
        &common::prepare_empty_dir("__test_22_context"),
        "test_peer_threshold",
        &tezos_env,
        patch_context,
        Some(common::p2p_cfg_with_threshold(
            NODE_P2P_CFG.clone(),
            0,
            peer_threshold_high,
            0,
        )),
        NODE_IDENTITY.clone(),
        (log, log_level),
        vec![],
        (false, false),
    )?;

    // register network channel listener
    let peers_mirror = Arc::new(RwLock::new(HashMap::new()));
    let _ = common::infra::test_actor::NetworkChannelListener::actor(
        &node.actor_system,
        node.network_channel.clone(),
        peers_mirror.clone(),
    );

    // wait for storage initialization to genesis
    node.wait_for_new_current_head(
        "genesis",
        node.tezos_env.genesis_header_hash()?,
        (Duration::from_secs(5), Duration::from_millis(250)),
    )?;

    // try connect several peers: <peer_threshold_high * 2>
    let incoming_peers: Vec<common::test_node_peer::TestNodePeer> = (0..(peer_threshold_high * 2))
        .map(|idx| format!("TEST_PEER_NODE_{}", idx))
        .map(|peer_name| {
            common::test_node_peer::TestNodePeer::connect(
                peer_name,
                NODE_P2P_CFG.0.listener_port,
                NODE_P2P_CFG.1.clone(),
                tezos_identity::Identity::generate(0f64).expect("failed to generate identity"),
                node.log.clone(),
                &node.tokio_runtime,
                common::test_cases_data::sandbox_branch_1_level3::serve_data,
            )
        })
        .collect();

    // give some time to async actors
    std::thread::sleep(Duration::from_secs(3));

    // verify peers state - we should accepted just max peer_threshold_high
    let connected = peers_mirror
        .read()
        .unwrap()
        .iter()
        .filter(|&(_, status)| {
            status == &common::infra::test_actor::PeerConnectionStatus::Connected
        })
        .count();
    assert_eq!(
        connected,
        peer_threshold_high / 2,
        "Actual connected count: {}, expected <= {}",
        connected,
        peer_threshold_high
    );

    drop(incoming_peers);

    Ok(())
}
