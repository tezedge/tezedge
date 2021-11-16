use std::time::{Duration, SystemTime};

use crypto::nonce::Nonce;
use shell_automaton::peers::check::timeouts::PeersCheckTimeoutsInitAction;
use shell_automaton::shell_compatibility_version::ShellCompatibilityVersion;
use shell_automaton::{Config, Quota, State};
use shell_automaton_testing::one_real_node_cluster::{Cluster, HandshakeError};
use shell_automaton_testing::service::IOCondition;
use tezos_identity::Identity;
use tezos_messages::p2p::encoding::ack::NackMotive;
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::version::NetworkVersion;

fn build_cluster(pow_target: f64) -> Cluster {
    let initial_time = SystemTime::now();

    let state = State::new(Config {
        initial_time,
        port: 9732,
        disable_mempool: false,
        private_node: false,
        pow_target,
        identity: Identity::generate(pow_target).unwrap(),
        shell_compatibility_version: ShellCompatibilityVersion::new(
            "TEZOS_LOCALNET".to_owned(),
            vec![1],
            vec![1],
        ),
        check_timeouts_interval: Duration::from_millis(500),
        peer_connecting_timeout: Duration::from_millis(2000),
        peer_handshaking_timeout: Duration::from_secs(8),
        peer_max_io_syscalls: 64,
        peers_potential_max: 2,
        peers_connected_max: 2,
        peers_graylist_disable: false,
        peers_graylist_timeout: Duration::from_secs(15 * 60),
        record_state_snapshots_with_interval: None,
        record_actions: false,
        quota: Quota {
            restore_duration_millis: 1000,
            read_quota: 1024,
            write_quota: 1024,
        },
    });

    Cluster::new(state, initial_time)
}

#[test]
fn test_can_handshake_outgoing() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    cluster.do_handshake(peer_id).unwrap();

    assert!(cluster
        .state()
        .peers
        .get(&peer_id.to_ipv4())
        .unwrap()
        .is_handshaked());
}

#[test]
fn test_can_handshake_incoming() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    cluster.do_handshake(peer_id).unwrap();

    assert!(cluster
        .state()
        .peers
        .get(&peer_id.to_ipv4())
        .unwrap()
        .is_handshaked());
}

#[test]
fn test_handshaking_nack_unknown_chain_name() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let peer = cluster.peer(peer_id);
    peer.set_read_cond(IOCondition::NoLimit)
        .set_write_cond(IOCondition::NoLimit);

    let sent_conn_msg = ConnectionMessage::try_new(
        peer_id.to_ipv4().port(),
        &peer.identity().public_key,
        &peer.identity().proof_of_work_stamp,
        Nonce::random(),
        NetworkVersion::new("TEZOS_UNKNOWN".to_owned(), 1, 1),
    )
    .unwrap();

    peer.send_conn_msg(&sent_conn_msg);
    cluster.dispatch_peer_ready_event(peer_id, true, true, false);

    let received_conn_msg = cluster.peer(peer_id).read_conn_msg();

    cluster
        .set_peer_crypto_with_conn_messages(peer_id, &sent_conn_msg, &received_conn_msg)
        .unwrap();

    let err = cluster.do_handshake(peer_id).err().expect(
        "handshaking was successful when it should have failed because of unknown chain name.",
    );
    match err {
        HandshakeError::Nack(info) => assert_eq!(*info.motive(), NackMotive::UnknownChainName),
        err => panic!("unexpected handshake error: {:?}", err),
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_nack_deprecated_distributed_version() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let peer = cluster.peer(peer_id);
    peer.set_read_cond(IOCondition::NoLimit)
        .set_write_cond(IOCondition::NoLimit);

    let sent_conn_msg = ConnectionMessage::try_new(
        peer_id.to_ipv4().port(),
        &peer.identity().public_key,
        &peer.identity().proof_of_work_stamp,
        Nonce::random(),
        NetworkVersion::new("TEZOS_LOCALNET".to_owned(), 0, 1),
    )
    .unwrap();

    peer.send_conn_msg(&sent_conn_msg);
    cluster.dispatch_peer_ready_event(peer_id, true, true, false);

    let received_conn_msg = cluster.peer(peer_id).read_conn_msg();

    cluster
        .set_peer_crypto_with_conn_messages(peer_id, &sent_conn_msg, &received_conn_msg)
        .unwrap();

    let err = cluster.do_handshake(peer_id).err().expect(
        "handshaking was successful when it should have failed because of deprecated db version.",
    );
    match err {
        HandshakeError::Nack(info) => {
            assert_eq!(*info.motive(), NackMotive::DeprecatedDistributedDbVersion)
        }
        err => panic!("unexpected handshake error: {:?}", err),
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_nack_deprecated_p2p_version() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let peer = cluster.peer(peer_id);
    peer.set_read_cond(IOCondition::NoLimit)
        .set_write_cond(IOCondition::NoLimit);

    let sent_conn_msg = ConnectionMessage::try_new(
        peer_id.to_ipv4().port(),
        &peer.identity().public_key,
        &peer.identity().proof_of_work_stamp,
        Nonce::random(),
        NetworkVersion::new("TEZOS_LOCALNET".to_owned(), 1, 0),
    )
    .unwrap();

    peer.send_conn_msg(&sent_conn_msg);
    cluster.dispatch_peer_ready_event(peer_id, true, true, false);

    let received_conn_msg = cluster.peer(peer_id).read_conn_msg();

    cluster
        .set_peer_crypto_with_conn_messages(peer_id, &sent_conn_msg, &received_conn_msg)
        .unwrap();

    let err = cluster.do_handshake(peer_id).err().expect(
        "handshaking was successful when it should have failed because of deprecated p2p version.",
    );
    match err {
        HandshakeError::Nack(info) => assert_eq!(*info.motive(), NackMotive::DeprecatedP2pVersion),
        err => panic!("unexpected handshake error: {:?}", err),
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_nack_already_connected_incoming() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);
    let peer_identity = cluster.peer(peer_id).identity().clone();

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    cluster.do_handshake(peer_id).unwrap();

    assert!(cluster
        .state()
        .peers
        .get(&peer_id.to_ipv4())
        .unwrap()
        .is_handshaked());

    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let err = cluster.do_handshake(peer_id).err().expect("handshaking was successful when it should have failed because peer with same identity is already connected.");

    match err {
        HandshakeError::Nack(info) => assert_eq!(*info.motive(), NackMotive::AlreadyConnected),
        err => panic!("unexpected handshake error: {:?}", err),
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_none());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_nack_already_connected_outgoing() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);
    let peer_identity = cluster.peer(peer_id).identity().clone();

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    cluster.do_handshake(peer_id).unwrap();

    assert!(cluster
        .state()
        .peers
        .get(&peer_id.to_ipv4())
        .unwrap()
        .is_handshaked());

    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let err = cluster.do_handshake(peer_id).err().expect("handshaking was successful when it should have failed because peer with same identity is already connected.");
    match err {
        HandshakeError::Nack(info) => assert_eq!(*info.motive(), NackMotive::AlreadyConnected),
        err => panic!("unexpected handshake error: {:?}", err),
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_none());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_nack_connecting_to_self_incoming() {
    let mut cluster = build_cluster(0.0);

    let peer_identity = cluster.state().config.identity.clone();
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let err = cluster.do_handshake(peer_id).err().expect("handshaking was successful when it should have failed because peer has the same identity as node.");
    match err {
        HandshakeError::Nack(info) => assert_eq!(*info.motive(), NackMotive::AlreadyConnected),
        err => panic!("unexpected handshake error: {:?}", err),
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_none());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_nack_connecting_to_self_outgoing() {
    let mut cluster = build_cluster(0.0);

    let peer_identity = cluster.state().config.identity.clone();
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let err = cluster.do_handshake(peer_id).err().expect("handshaking was successful when it should have failed because peer has the same identity as node.");
    match err {
        HandshakeError::Nack(info) => assert_eq!(*info.motive(), NackMotive::AlreadyConnected),
        err => panic!("unexpected handshake error: {:?}", err),
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_none());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_bad_pow_incoming() {
    let mut cluster = build_cluster(10.0);

    let peer_identity = loop {
        // generate identity which doesn't pass 10.0 pow target.
        let identity = Identity::generate(0.0).unwrap();
        if identity
            .proof_of_work_stamp
            .check(&identity.public_key, 10.0)
            .is_err()
        {
            break identity;
        }
    };
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let err = cluster
        .do_handshake(peer_id)
        .err()
        .expect("handshaking was successful when it should have failed because peer has bad pow.");
    match err {
        HandshakeError::NackV0 => panic!(),
        HandshakeError::Nack(_) => panic!(),
        HandshakeError::NotConnected => {}
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_handshaking_bad_pow_outgoing() {
    let mut cluster = build_cluster(10.0);

    let peer_identity = loop {
        // generate identity which doesn't pass 10.0 pow target.
        let identity = Identity::generate(0.0).unwrap();
        if identity
            .proof_of_work_stamp
            .check(&identity.public_key, 10.0)
            .is_err()
        {
            break identity;
        }
    };
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let err = cluster
        .do_handshake(peer_id)
        .err()
        .expect("handshaking was successful when it should have failed because peer has bad pow.");
    match err {
        HandshakeError::NackV0 => panic!(),
        HandshakeError::Nack(_) => panic!(),
        HandshakeError::NotConnected => {}
    }

    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
}

#[test]
fn test_connecting_timeout_incoming() {
    let mut cluster = build_cluster(0.0);
    let check_timeouts_interval = cluster.state().config.check_timeouts_interval;
    let configured_timeout = cluster.state().config.peer_connecting_timeout;

    let peer_identity = cluster.state().config.identity.clone();
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_from_peer(peer_id);

    for _ in 1..(configured_timeout.as_nanos() / check_timeouts_interval.as_nanos()).max(1) {
        cluster.advance_time(check_timeouts_interval);
        cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());
    }
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_some());

    cluster.advance_time(check_timeouts_interval);
    cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());

    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
}

#[test]
fn test_connecting_timeout_outgoing() {
    let mut cluster = build_cluster(0.0);
    let check_timeouts_interval = cluster.state().config.check_timeouts_interval;
    let configured_timeout = cluster.state().config.peer_connecting_timeout;

    let peer_identity = cluster.state().config.identity.clone();
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_to_peer(peer_id);

    for _ in 1..(configured_timeout.as_nanos() / check_timeouts_interval.as_nanos()).max(1) {
        cluster.advance_time(check_timeouts_interval);
        cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());
    }
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_some());

    cluster.advance_time(check_timeouts_interval);
    cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());

    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
}

#[test]
fn test_handshaking_timeout_incoming() {
    let mut cluster = build_cluster(0.0);
    let check_timeouts_interval = cluster.state().config.check_timeouts_interval;
    let configured_timeout = cluster.state().config.peer_handshaking_timeout;

    let peer_identity = cluster.state().config.identity.clone();
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_from_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    for _ in 1..(configured_timeout.as_nanos() / check_timeouts_interval.as_nanos()).max(1) {
        cluster.advance_time(check_timeouts_interval);
        cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());
    }
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_some());

    cluster.advance_time(check_timeouts_interval);
    cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());

    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
}

#[test]
fn test_handshaking_timeout_outgoing() {
    let mut cluster = build_cluster(0.0);
    let check_timeouts_interval = cluster.state().config.check_timeouts_interval;
    let configured_timeout = cluster.state().config.peer_handshaking_timeout;

    let peer_identity = cluster.state().config.identity.clone();
    let peer_id = cluster.peer_init_with_identity(peer_identity);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    for _ in 1..(configured_timeout.as_nanos() / check_timeouts_interval.as_nanos()).max(1) {
        cluster.advance_time(check_timeouts_interval);
        cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());
    }
    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_some());

    cluster.advance_time(check_timeouts_interval);
    cluster.dispatch(PeersCheckTimeoutsInitAction {}.into());

    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
}

#[test]
fn test_handshaking_connection_message_0_bytes_chunk() {
    let mut cluster = build_cluster(0.0);

    let peer_id = cluster.peer_init(0.0);

    cluster.connect_to_peer(peer_id);
    cluster.set_peer_connected(peer_id);

    let peer = cluster.peer(peer_id);
    peer.set_write_cond(IOCondition::NoLimit);
    peer.set_read_cond(IOCondition::NoLimit);
    peer.send_bytes(&[0, 0]);
    cluster.dispatch_peer_ready_event(peer_id, true, true, false);

    assert!(cluster.state().peers.get(&peer_id.to_ipv4()).is_none());
    assert!(cluster
        .state()
        .peers
        .get_blacklisted_ip(&peer_id.to_ipv4().ip())
        .is_some());
}
