// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use itertools::Itertools;
use quickcheck::{Arbitrary, Gen};
use std::io;
use std::time::{Duration, SystemTime};

use crypto::nonce::Nonce;
use tezedge_state::acceptors::*;
use tezedge_state::proposals::peer_handshake_message::PeerDecodedHanshakeMessage;
use tezedge_state::proposals::peer_handshake_message::*;
use tezedge_state::proposals::*;
use tezedge_state::*;
use tezos_identity::Identity;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryWrite};
use tezos_messages::p2p::encoding::ack::{AckMessage, NackInfo};
use tezos_messages::p2p::encoding::prelude::{ConnectionMessage, MetadataMessage};

#[derive(Debug, Clone)]
enum FakeWritable {
    NoLimit,
    // Limit(usize),
    Error(io::ErrorKind),
}

impl io::Read for FakeWritable {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}

impl io::Write for FakeWritable {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            Self::NoLimit => Ok(buf.len()),
            // Self::Limit(limit) => Ok(buf.len().min(*limit)),
            Self::Error(kind) => Err(io::Error::new(*kind, "simulated error")),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum Messages {
    Handshake(PeerDecodedHanshakeMessage),
    Writable(FakeWritable),
    Disconnected,
}

impl From<PeerDecodedHanshakeMessage> for Messages {
    fn from(msg: PeerDecodedHanshakeMessage) -> Self {
        Messages::Handshake(msg)
    }
}

impl From<FakeWritable> for Messages {
    fn from(writable: FakeWritable) -> Self {
        Messages::Writable(writable)
    }
}

struct StateWithEffects<Efs> {
    effects: Efs,
    state: TezedgeState,
}

impl<Efs: Clone> Clone for StateWithEffects<Efs> {
    fn clone(&self) -> Self {
        Self {
            effects: self.effects.clone(),
            state: self.state.clone(),
        }
    }
}

fn to_binary_chunk<M: BinaryWrite>(msg: &M) -> BinaryChunk {
    BinaryChunk::from_content(&msg.as_bytes().unwrap()).unwrap()
}

fn build_messages(g: &mut Gen, identity: &Identity) -> Vec<Messages> {
    let conn_msg = ConnectionMessage::try_new(
        0,
        &identity.public_key,
        &identity.proof_of_work_stamp,
        Nonce::random(),
        sample_tezedge_state::default_shell_compatibility_version().to_network_version(),
    )
    .unwrap();
    let meta_msg = MetadataMessage::new(false, true);
    let ack_msg = AckMessage::Ack;
    let nack_v0_msg = AckMessage::NackV0;
    let nack_msg = AckMessage::Nack(NackInfo::arbitrary(g));

    vec![
        FakeWritable::NoLimit.into(),
        FakeWritable::Error(io::ErrorKind::BrokenPipe).into(),
        FakeWritable::NoLimit.into(),
        FakeWritable::Error(io::ErrorKind::BrokenPipe).into(),
        FakeWritable::NoLimit.into(),
        FakeWritable::Error(io::ErrorKind::BrokenPipe).into(),
        PeerDecodedHanshakeMessage::new(to_binary_chunk(&conn_msg), conn_msg.into()).into(),
        PeerDecodedHanshakeMessage::new(to_binary_chunk(&meta_msg), meta_msg.into()).into(),
        PeerDecodedHanshakeMessage::new(to_binary_chunk(&ack_msg), ack_msg.into()).into(),
        PeerDecodedHanshakeMessage::new(to_binary_chunk(&nack_v0_msg), nack_v0_msg.into()).into(),
        PeerDecodedHanshakeMessage::new(to_binary_chunk(&nack_msg), nack_msg.into()).into(),
        Messages::Disconnected,
    ]
}

fn should_sequence_fail(seq: &Vec<Messages>, incoming: bool) -> bool {
    if seq.len() != 6 {
        return true;
    }

    use Messages::*;

    match (incoming, &seq[0..2]) {
        (true, [Handshake(msg), Writable(FakeWritable::NoLimit)])
        | (false, [Handshake(msg), Writable(FakeWritable::NoLimit)])
        | (false, [Writable(FakeWritable::NoLimit), Handshake(msg)])
            if matches!(
                msg.message_type(),
                PeerDecodedHanshakeMessageType::Connection(_)
            ) => {}
        _ => return true,
    }

    match &seq[2..4] {
        [Handshake(msg), Writable(FakeWritable::NoLimit)]
        | [Writable(FakeWritable::NoLimit), Handshake(msg)]
            if matches!(
                msg.message_type(),
                PeerDecodedHanshakeMessageType::Metadata(_)
            ) => {}
        _ => return true,
    }

    match &seq[4..6] {
        [Handshake(msg), Writable(FakeWritable::NoLimit)]
        | [Writable(FakeWritable::NoLimit), Handshake(msg)]
            if matches!(
                msg.message_type(),
                PeerDecodedHanshakeMessageType::Ack(AckMessage::Ack)
            ) => {}
        _ => return true,
    }

    false
}

fn try_sequence<Efs: Effects + Clone>(
    state_with_effects: &mut StateWithEffects<Efs>,
    sequence: &[Messages],
    _incoming: bool,
) -> bool {
    let state = &mut state_with_effects.state;
    let effects = &mut state_with_effects.effects;

    let peer = PeerAddress::ipv4_from_index(1);

    for msg in sequence {
        match msg {
            Messages::Handshake(msg) => state.accept(PeerHandshakeMessageProposal {
                effects,
                time_passed: Duration::from_millis(1),
                peer: peer.clone(),
                message: msg.clone(),
            }),
            Messages::Writable(writable) => state.accept(PeerWritableProposal {
                effects,
                time_passed: Duration::from_millis(1),
                peer: peer.clone(),
                stream: &mut writable.clone(),
            }),
            Messages::Disconnected => state.accept(PeerDisconnectedProposal {
                effects,
                time_passed: Duration::from_millis(1),
                peer: peer.clone(),
            }),
        };
    }
    state.accept(TickProposal {
        effects,
        time_passed: state.config().peer_timeout,
    });
    state.assert_state();

    let is_connected = state.is_peer_connected(&peer);
    assert_ne!(is_connected, state.is_address_blacklisted(&peer), "at the end of trying sequence, peer should either be blacklisted or connected, it wasn't! sequence: {}, state: {:?}", sequence_to_str(sequence), state);

    is_connected
}

fn sequence_to_str(seq: &[Messages]) -> String {
    let seq_str = seq
        .iter()
        .map(|msg| match msg {
            Messages::Handshake(msg) => match msg.message_type() {
                PeerDecodedHanshakeMessageType::Connection(_) => "receive_connect",
                PeerDecodedHanshakeMessageType::Metadata(_) => "receive_metadata",
                PeerDecodedHanshakeMessageType::Ack(AckMessage::Ack) => "receive_ack",
                PeerDecodedHanshakeMessageType::Ack(AckMessage::NackV0) => "receive_nack_v0",
                PeerDecodedHanshakeMessageType::Ack(AckMessage::Nack(_)) => "receive_nack",
            }
            .to_owned(),
            Messages::Writable(writable) => format!("Writable({:?})", writable),
            Messages::Disconnected => "disconnected".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{}]", seq_str)
}

fn fork_state_and_init_incoming<Efs: Effects + Clone>(
    effects: &Efs,
    state: &TezedgeState,
    peer_addr: PeerAddress,
) -> StateWithEffects<Efs> {
    let mut effects = effects.clone();
    let mut state = state.clone();
    state.accept(NewPeerConnectProposal {
        effects: &mut effects,
        time_passed: Duration::new(0, 0),
        peer: peer_addr,
    });

    StateWithEffects { effects, state }
}

fn fork_state_and_init_outgoing<Efs: Effects + Clone>(
    effects: &Efs,
    state: &TezedgeState,
    peer_addr: PeerAddress,
    success: bool,
) -> StateWithEffects<Efs> {
    let mut effects = effects.clone();
    let mut state = state.clone();
    state.accept(ExtendPotentialPeersProposal {
        effects: &mut effects,
        time_passed: Duration::new(0, 0),
        peers: std::iter::once(peer_addr.into()),
    });
    let mut requests = vec![];
    state.get_requests(&mut requests);

    let mut req_found = false;
    for req in requests {
        match req {
            TezedgeRequest::ConnectPeer { req_id, peer } if peer == peer_addr => {
                req_found = true;
                state.accept(PendingRequestProposal {
                    effects: &mut effects,
                    time_passed: Duration::new(0, 0),
                    req_id,
                    message: match success {
                        true => PendingRequestMsg::ConnectPeerSuccess,
                        false => PendingRequestMsg::ConnectPeerError,
                    },
                });
            }
            _ => {}
        }
    }
    assert!(req_found);

    StateWithEffects { effects, state }
}

#[test]
fn simulate_one_peer_all_message_sequences() {
    let mut g = Gen::new(10);
    println!("generating identity for client...");
    let address = PeerAddress::ipv4_from_index(1);
    let identity = Identity::generate(1.0).unwrap();
    println!("generating identity for p2p manager...");
    let node_identity = Identity::generate(1.0).unwrap();

    let mut successful_sequences = vec![];

    let msgs = build_messages(&mut g, &identity);

    let initial_time = SystemTime::now();
    let mut effects = DefaultEffects::default();
    let tezedge_state = TezedgeState::new(
        slog::Logger::root(slog::Discard, slog::o!()),
        TezedgeConfig {
            port: 0,
            disable_mempool: false,
            private_node: false,
            disable_quotas: false,
            disable_blacklist: false,
            min_connected_peers: 10,
            max_connected_peers: 20,
            max_potential_peers: 100,
            max_pending_peers: 20,
            periodic_react_interval: Duration::from_millis(1),
            reset_quotas_interval: Duration::from_secs(5),
            peer_blacklist_duration: Duration::from_secs(30 * 60),
            peer_timeout: Duration::from_secs(8),
            pow_target: 1.0,
        },
        node_identity.clone(),
        sample_tezedge_state::default_shell_compatibility_version(),
        &mut effects,
        initial_time,
        sample_tezedge_state::main_chain_id(),
    );

    let state_incoming = fork_state_and_init_incoming(&effects, &tezedge_state, address);
    let state_outgoing = fork_state_and_init_outgoing(&effects, &tezedge_state, address, true);
    let state_outgoing_failed =
        fork_state_and_init_outgoing(&effects, &tezedge_state, address, false);

    for seq_len in 1..=6 {
        println!("trying sequences with length: {}", seq_len);
        let mut count = 0;
        for seq in msgs.clone().into_iter().permutations(seq_len) {
            count += 1;

            if try_sequence(&mut state_outgoing_failed.clone(), &seq, false) {
                panic!("sequence with failed outgoing connection succeeded (shouldn't have!). sequence: {:?}", seq);
            }

            let should_fail = should_sequence_fail(&seq, false);
            let result = try_sequence(&mut state_outgoing.clone(), &seq, false);
            assert_eq!(
                result,
                !should_fail,
                "unexpected result for (outgoing) sequence: {}",
                sequence_to_str(&seq)
            );

            if result {
                println!("successful outgoing sequence: {}", sequence_to_str(&seq));
                successful_sequences.push(seq.clone());
            }

            let should_fail = should_sequence_fail(&seq, true);
            let result = try_sequence(&mut state_incoming.clone(), &seq, true);
            assert_eq!(
                result,
                !should_fail,
                "unexpected result for (incoming) sequence: {}",
                sequence_to_str(&seq)
            );

            if result {
                println!("successful incoming sequence: {}", sequence_to_str(&seq));
                successful_sequences.push(seq.clone());
            }

            // try to test disconnection at the end
            let mut seq = seq;
            seq.push(Messages::Disconnected);

            assert_eq!(
                try_sequence(&mut state_outgoing.clone(), &seq, false),
                false,
                "unexpected result for (outgoing) sequence: {}. After disconnection in the end, it should have failed!",
                sequence_to_str(&seq)
            );

            assert_eq!(
                try_sequence(&mut state_incoming.clone(), &seq, true),
                false,
                "unexpected result for (incoming) sequence: {}. After disconnection in the end, it should have failed!",
                sequence_to_str(&seq)
            );
        }
        println!("tried permutations: {}", count);
    }
}
