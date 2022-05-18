use std::convert::TryInto;
use std::{
    convert::TryFrom,
    time::{Duration, SystemTime},
};

use crypto::hash::ChainId;
use shell_automaton::config::default_test_config;
use shell_automaton::service::storage_service::{StorageError, StorageResponseError};
use shell_automaton::{
    current_head::CurrentHeadState,
    event::WakeupEvent,
    service::storage_service::{StorageRequestPayload, StorageResponseSuccess},
    shell_compatibility_version::ShellCompatibilityVersion,
};
use shell_automaton::{Config, Service, State};
use shell_automaton_testing::service::IOCondition;
use shell_automaton_testing::{build_expected_history, generate_chain};
use shell_automaton_testing::{one_real_node_cluster::Cluster, service::StorageResponse};
use storage::BlockHeaderWithHash;
use tezos_identity::Identity;
use tezos_messages::p2p::encoding::block_header::Level;
use tezos_messages::p2p::encoding::peer::PeerMessage;
use tezos_messages::p2p::encoding::prelude::{BlockLocator, GetCurrentBranchMessage};

fn data(current_head_level: Level) -> (Cluster, Vec<BlockHeaderWithHash>) {
    let initial_time = SystemTime::now();

    let mut state = State::new(Config {
        initial_time,
        pow_target: 0.0,
        identity: Identity::generate(0.0).unwrap(),
        shell_compatibility_version: ShellCompatibilityVersion::new(
            "TEZOS_LOCALNET".to_owned(),
            vec![1],
            vec![1],
        ),
        chain_id: ChainId::try_from("NetXz969SFaFn8k").unwrap(), // granada
        check_timeouts_interval: Duration::from_millis(500),
        peer_connecting_timeout: Duration::from_millis(2000),
        peer_handshaking_timeout: Duration::from_secs(8),
        peers_potential_max: 2,
        peers_connected_max: 2,
        peers_graylist_disable: false,
        peers_graylist_timeout: Duration::from_secs(15 * 60),
        ..default_test_config()
    });
    let genesis_header = state
        .config
        .protocol_runner
        .environment
        .genesis_header(
            "CoV8SQumiVU9saiu3FVNeDNewJaJH8yWdsGF3WLdsRr2P9S7MzCj"
                .try_into()
                .unwrap(),
            "LLoZS2LW3rEi7KYU4ouBQtorua37aWWCtpDmv1n2x3xoKi6sVXLWp"
                .try_into()
                .unwrap(),
        )
        .unwrap();
    let genesis_block = BlockHeaderWithHash {
        hash: state
            .config
            .init_storage_data
            .genesis_block_header_hash
            .clone(),
        header: genesis_header.into(),
    };
    let chain = generate_chain(genesis_block, current_head_level);
    let head = chain.last().unwrap().clone();
    let head_pred = chain.iter().rev().nth(1).cloned();
    state.current_head = CurrentHeadState::rehydrated(head, head_pred);

    (Cluster::new(state, initial_time), chain)
}

/// Test GetCurrentBranch response from the node.
///
/// # Parameters:
/// - `current_head_level`: configure current head level of the node.
/// - `missing_bellow_level`: level, bellow which, we will return `None`
///   for storage requests. This is to simulate case, when we don't have
///   block header available in storage.
/// - `error_at_level`: level, bellow which, we will return error for
///   storage requests.
fn test(
    current_head_level: Level,
    missing_bellow_level: Option<Level>,
    error_bellow_level: Option<Level>,
) {
    let (mut cluster, chain) = data(current_head_level);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    cluster.do_handshake(peer_id).unwrap();

    let chain_id = cluster.state().config.chain_id.clone();
    let node_pkh = cluster.state().config.identity.peer_id();
    let peer_pkh = cluster.peer(peer_id).identity().peer_id();
    let expected_current_head = cluster.state().current_head.get().unwrap().clone();
    let expected_history = build_expected_history(&chain, node_pkh, peer_pkh, current_head_level);

    let peer = cluster.peer(peer_id);
    peer.send_peer_message(PeerMessage::GetCurrentBranch(GetCurrentBranchMessage::new(
        chain_id,
    )));
    peer.set_read_cond(IOCondition::NoLimit)
        .set_write_cond(IOCondition::NoLimit);
    cluster.dispatch_peer_ready_event(peer_id, true, true, false);

    eprintln!("Expected CurrentHead: {:?}", expected_current_head);
    eprintln!("Expected History:");
    for block in expected_history.iter() {
        eprintln!("({} - {})", block.header.level(), block.hash);
    }
    eprintln!("----------------\n");

    let mut was_min_level_reached = false;
    for block in expected_history.iter() {
        if was_min_level_reached {
            break;
        }
        let service = cluster.service().storage();
        loop {
            let req = service
                .requests
                .pop_front()
                .expect("Expected storage request from state machine");
            match req.payload {
                StorageRequestPayload::BlockHashByLevelGet(level) => {
                    assert_eq!(
                        level,
                        block.header.level(),
                        "State machine requested invalid level for current branch!"
                    );
                    let result = if missing_bellow_level.filter(|l| level <= *l).is_some() {
                        was_min_level_reached = true;
                        Ok(StorageResponseSuccess::BlockHashByLevelGetSuccess(None))
                    } else if error_bellow_level.filter(|l| level <= *l).is_some() {
                        was_min_level_reached = true;
                        Err(StorageResponseError::BlockHashByLevelGetError(
                            StorageError::mocked(),
                        ))
                    } else {
                        Ok(StorageResponseSuccess::BlockHashByLevelGetSuccess(Some(
                            block.hash.clone(),
                        )))
                    };
                    service.responses.push_back(StorageResponse {
                        req_id: req.id,
                        result,
                    });
                    cluster.dispatch(WakeupEvent);
                    break;
                }
                _ => continue,
            }
        }
    }

    eprintln!("State: {:?}", cluster.state());
    let current_branch = loop {
        let msg = cluster
            .peer(peer_id)
            .read_peer_message()
            .expect("Expected CurrentBranch response");
        match msg {
            PeerMessage::CurrentBranch(msg) => break msg.current_branch().clone(),
            _ => {
                cluster.dispatch_peer_ready_event(peer_id, true, true, false);
                continue;
            }
        };
    };

    let expected_current_branch = {
        let min_level = missing_bellow_level.or(error_bellow_level).unwrap_or(-1);
        let history = expected_history
            .into_iter()
            .filter(|b| b.header.level() > min_level)
            .map(|b| b.hash)
            .collect();

        BlockLocator::new((*expected_current_head.header).clone(), history)
    };

    assert_eq!(current_branch, expected_current_branch);
}

#[test]
fn test_get_current_branch_current_head_0() {
    test(0, None, None);
}

/// Should always succeed as storage shouldn't even be touched when
/// current head is 0.
#[test]
fn test_get_current_branch_current_head_0_error() {
    test(0, None, Some(0));
}

/// Should always succeed as storage shouldn't even be touched when
/// current head is 0.
#[test]
fn test_get_current_branch_current_head_0_missing() {
    test(0, Some(0), None);
}

#[test]
fn test_get_current_branch_current_head_1_to_100() {
    for i in 1..=100 {
        test(i, None, None);
    }
}

#[test]
fn test_get_current_branch_current_head_1_to_100_error() {
    for i in 1..=10 {
        for j in 0..=i {
            test(i, None, Some(j));
        }
    }
}

#[test]
fn test_get_current_branch_current_head_1_to_100_missing() {
    for i in 1..=10 {
        for j in 0..=i {
            test(i, Some(j), None);
        }
    }
}
