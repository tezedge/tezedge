use std::convert::TryInto;
use std::{
    convert::TryFrom,
    time::{Duration, SystemTime},
};

use crypto::hash::ChainId;
use shell_automaton::bootstrap::{BootstrapError, BootstrapState};
use shell_automaton::config::default_test_config;
use shell_automaton::mempool::HeadState;
use shell_automaton::{
    current_head::CurrentHeadState, shell_compatibility_version::ShellCompatibilityVersion,
};
use shell_automaton::{Config, State};
use shell_automaton_testing::one_real_node_cluster::Cluster;
use shell_automaton_testing::service::IOCondition;
use shell_automaton_testing::{generate_chain, generate_next_block};
use storage::BlockHeaderWithHash;
use tezos_identity::Identity;
use tezos_messages::p2p::encoding::block_header::{GetBlockHeadersMessage, Level};
use tezos_messages::p2p::encoding::current_head::CurrentHeadMessage;
use tezos_messages::p2p::encoding::mempool::Mempool;
use tezos_messages::p2p::encoding::operations_for_blocks::{
    GetOperationsForBlocksMessage, OperationsForBlock, OperationsForBlocksMessage, Path,
};
use tezos_messages::p2p::encoding::peer::PeerMessage;
use tezos_messages::p2p::encoding::prelude::GetCurrentBranchMessage;

fn data(chain_level: Level) -> (Cluster, Vec<BlockHeaderWithHash>) {
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
        peers_bootstrapped_min: 1,
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
    let chain = generate_chain(genesis_block, chain_level);
    state.current_head = CurrentHeadState::Rehydrated {
        head: chain.last().unwrap().clone(),
        head_pred: chain.iter().rev().nth(1).cloned(),
    };
    state.bootstrap = BootstrapState::Finished {
        time: 0,
        error: None,
    };
    state.mempool.local_head_state = Some(HeadState {
        header: (*chain.last().unwrap().header).clone(),
        hash: chain.last().unwrap().hash.clone(),
        prevalidator_ready: true,

        metadata_hash: None,
        ops_metadata_hash: None,
    });

    (Cluster::new(state, initial_time), chain)
}

fn test(chain_level: Level, fork_depth: usize, should_accept: bool) {
    let (mut cluster, chain) = data(chain_level);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    cluster.do_handshake(peer_id).unwrap();

    let chain_id = cluster.state().config.chain_id.clone();

    let forked_block = dbg!(generate_next_block(
        chain.iter().rev().nth(fork_depth).unwrap(),
        30000
    ));
    let forked_chain = std::iter::once(forked_block.clone())
        .chain((0..fork_depth).scan(forked_block, |pred, _| {
            let block = generate_next_block(pred, 30000);
            *pred = block.clone();
            Some(block)
        }))
        .collect::<Vec<_>>();
    dbg!(&forked_chain);

    for block in forked_chain.iter() {
        let peer = cluster.peer(peer_id);
        peer.send_peer_message(PeerMessage::CurrentHead(CurrentHeadMessage::new(
            chain_id.clone(),
            (*block.header).clone(),
            Mempool::new(vec![], vec![]),
        )));
        peer.set_read_cond(IOCondition::NoLimit)
            .set_write_cond(IOCondition::NoLimit);
        cluster.dispatch_peer_ready_event(peer_id, true, true, false);
    }
    let peer = cluster.peer(peer_id);
    assert_eq!(peer.read_peer_message(), Some(PeerMessage::Bootstrap));
    // we shouldn't accept another block at same level.
    assert_eq!(
        peer.read_peer_message(),
        Some(PeerMessage::GetCurrentBranch(GetCurrentBranchMessage::new(
            chain_id.clone()
        )))
    );

    for block in forked_chain.iter().rev().skip(1) {
        let peer = cluster.peer(peer_id);
        if let Some(msg) = peer.read_peer_message() {
            let expected_msg = GetBlockHeadersMessage::new(vec![block.hash.clone()]);
            let expected_msg = PeerMessage::GetBlockHeaders(expected_msg);
            if should_accept {
                assert_eq!(msg, expected_msg);
            }
            if msg == expected_msg {
                peer.send_peer_message(PeerMessage::BlockHeader((*block.header).clone().into()));
                peer.set_read_cond(IOCondition::NoLimit)
                    .set_write_cond(IOCondition::NoLimit);
                cluster.dispatch_peer_ready_event(peer_id, true, true, false);
            }
        }
    }

    if !should_accept {
        assert_eq!(cluster.peer(peer_id).read_peer_message(), None);
        if let BootstrapState::Finished { error, .. } = &cluster.state().bootstrap {
            dbg!(error);
            assert!(matches!(
                error,
                Some(BootstrapError::CementedBlockReorg { .. })
            ));
        }
        return;
    }

    for block in forked_chain.iter() {
        let peer = cluster.peer(peer_id);
        let key_1 = OperationsForBlock::new(block.hash.clone(), 0);
        let key_2 = OperationsForBlock::new(block.hash.clone(), 1);
        let key_3 = OperationsForBlock::new(block.hash.clone(), 2);
        let key_4 = OperationsForBlock::new(block.hash.clone(), 3);
        assert_eq!(
            peer.read_peer_message(),
            Some(PeerMessage::GetOperationsForBlocks(
                GetOperationsForBlocksMessage::new(vec![
                    key_1.clone(),
                    key_2.clone(),
                    key_3.clone(),
                    key_4.clone(),
                ])
            ))
        );
        peer.send_peer_message(PeerMessage::OperationsForBlocks(
            OperationsForBlocksMessage::new(key_1, Path(vec![]), vec![]),
        ));
        peer.send_peer_message(PeerMessage::OperationsForBlocks(
            OperationsForBlocksMessage::new(key_2, Path(vec![]), vec![]),
        ));
        peer.send_peer_message(PeerMessage::OperationsForBlocks(
            OperationsForBlocksMessage::new(key_3, Path(vec![]), vec![]),
        ));
        peer.send_peer_message(PeerMessage::OperationsForBlocks(
            OperationsForBlocksMessage::new(key_4, Path(vec![]), vec![]),
        ));
        peer.set_read_cond(IOCondition::NoLimit)
            .set_write_cond(IOCondition::NoLimit);
        cluster.dispatch_peer_ready_event(peer_id, true, true, false);
    }

    // TODO(zura): uncomment once we have block application mocked.
    // assert_eq!(
    //     cluster.state().current_head.get().unwrap().hash,
    //     forked_block_2.hash
    // );
}

#[test]
fn test_bootstrap_allowed_fork() {
    for i in 3..10 {
        test(i, 1, true);
        test(i, 2, true);
    }
}

#[test]
fn test_bootstrap_cemented_block_fork_should_be_rejected() {
    for i in 3..10 {
        test(20, i, false);
    }
}
