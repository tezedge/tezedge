// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::time::{Duration, Instant, SystemTime};

use crypto::crypto_box::{CryptoKey, PublicKey};
use crypto::nonce::Nonce;

use shell_automaton::event::{P2pPeerEvent, P2pServerEvent};
use shell_automaton::peer::PeerCrypto;
use shell_automaton::peers::add::multi::PeersAddMultiAction;
use shell_automaton::{
    check_timeouts, effects, reducer, Action, EnablingCondition, MioWaitForEventsAction, State,
    Store,
};
use tezos_identity::Identity;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryWrite};
use tezos_messages::p2p::encoding::ack::{AckMessage, NackInfo};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::metadata::MetadataMessage;

use crate::service::{
    ActorsServiceDummy, BakerServiceDummy, ConnectedState, DnsServiceMocked, IOCondition,
    MioPeerMockedId, MioPeerStreamMocked, MioServiceMocked, ProtocolRunnerServiceDummy,
    RandomnessServiceMocked, RpcServiceDummy, StorageServiceDummy,
};
use crate::service::{Service, TimeService};

#[derive(Debug, Eq, PartialEq)]
pub enum HandshakeError {
    NotConnected,
    NackV0,
    Nack(NackInfo),
}

#[derive(Clone)]
pub struct ServiceMocked {
    time: Instant,
    pub randomness: RandomnessServiceMocked,
    pub dns: DnsServiceMocked,
    pub mio: MioServiceMocked,
    pub protocol_runner: ProtocolRunnerServiceDummy,
    pub storage: StorageServiceDummy,
    pub rpc: RpcServiceDummy,
    pub actors: ActorsServiceDummy,
    pub baker: BakerServiceDummy,
}

impl ServiceMocked {
    pub fn new() -> Self {
        Self {
            time: Instant::now(),
            randomness: RandomnessServiceMocked::Dummy,
            dns: DnsServiceMocked::Constant(Ok(vec![])),
            mio: MioServiceMocked::new(([0, 0, 0, 0], 9732).into(), u16::MAX as usize),
            protocol_runner: ProtocolRunnerServiceDummy::new(),
            storage: StorageServiceDummy::new(),
            rpc: RpcServiceDummy::new(),
            actors: ActorsServiceDummy::new(),
            baker: BakerServiceDummy::default(),
        }
    }

    pub fn advance_time(&mut self, by: Duration) {
        self.time += by;
    }
}

impl Default for ServiceMocked {
    fn default() -> Self {
        Self::new()
    }
}

impl TimeService for ServiceMocked {
    fn monotonic_time(&mut self) -> Instant {
        self.time
    }
}

impl Service for ServiceMocked {
    type Randomness = RandomnessServiceMocked;
    type Dns = DnsServiceMocked;
    type Mio = MioServiceMocked;
    type ProtocolRunner = ProtocolRunnerServiceDummy;
    type Storage = StorageServiceDummy;
    type Rpc = RpcServiceDummy;
    type Actors = ActorsServiceDummy;
    type Baker = BakerServiceDummy;

    fn randomness(&mut self) -> &mut Self::Randomness {
        &mut self.randomness
    }

    fn dns(&mut self) -> &mut Self::Dns {
        &mut self.dns
    }

    fn mio(&mut self) -> &mut Self::Mio {
        &mut self.mio
    }

    fn protocol_runner(&mut self) -> &mut Self::ProtocolRunner {
        &mut self.protocol_runner
    }

    fn storage(&mut self) -> &mut Self::Storage {
        &mut self.storage
    }

    fn rpc(&mut self) -> &mut Self::Rpc {
        &mut self.rpc
    }

    fn actors(&mut self) -> &mut Self::Actors {
        &mut self.actors
    }

    fn prevalidator(&mut self) -> &mut Self::ProtocolRunner {
        self.protocol_runner()
    }

    fn baker(&mut self) -> &mut Self::Baker {
        &mut self.baker
    }
}

#[derive(Clone)]
pub struct Cluster {
    store: Store<ServiceMocked>,
}

impl Cluster {
    pub fn new(initial_state: State, initial_time: SystemTime) -> Self {
        Self {
            store: Store::new(
                reducer,
                |store, action| {
                    eprintln!("[+] Action: {:#?}", &action);
                    eprintln!("[+] State: {:#?}\n", store.state().peers);
                    effects(store, action)
                },
                ServiceMocked::new(),
                initial_time,
                initial_state,
            ),
        }
    }

    pub fn advance_time(&mut self, by: Duration) {
        self.store.service.advance_time(by)
    }

    pub fn state(&self) -> &State {
        self.store.state()
    }

    pub fn service(&mut self) -> &mut ServiceMocked {
        self.store.service()
    }

    pub fn dispatch<T>(&mut self, action: T) -> bool
    where
        T: Into<Action> + EnablingCondition<State>,
    {
        self.store.dispatch(action)
    }

    /// End current simulated `make_progress` loop's current iteration.
    pub fn loop_next(&mut self) {
        self.dispatch(MioWaitForEventsAction {});
        check_timeouts(&mut self.store);
    }

    pub fn dispatch_peer_ready_event(
        &mut self,
        peer_id: MioPeerMockedId,
        is_readable: bool,
        is_writable: bool,
        is_closed: bool,
    ) -> bool {
        self.dispatch(P2pPeerEvent {
            token: peer_id.to_token(),
            address: peer_id.to_ipv4(),
            is_readable,
            is_writable,
            is_closed,
        })
    }

    pub fn peer_init(&mut self, pow_target: f64) -> MioPeerMockedId {
        self.store.service.mio().peer_init(pow_target)
    }

    pub fn peer_init_with_identity(&mut self, identity: Identity) -> MioPeerMockedId {
        self.store.service.mio().peer_init_with_identity(identity)
    }

    /// Panics if peer with such id is not found.
    pub fn peer(&mut self, id: MioPeerMockedId) -> &mut MioPeerStreamMocked {
        &mut self.store.service.mio().peer(id).stream
    }

    pub fn connect_from_peer(&mut self, peer_id: MioPeerMockedId) {
        match self.peer(peer_id).conn_state_mut() {
            conn_state @ ConnectedState::Disconnected => {
                *conn_state = ConnectedState::OutgoingInit;
            }
            conn_state => panic!(
                "unexpected peer conn_state when requesting to connect from peer to node: {:?}",
                conn_state
            ),
        }
        self.store.service.mio().backlog_push(peer_id);

        self.dispatch(P2pServerEvent {});
    }

    pub fn connect_to_peer(&mut self, peer_id: MioPeerMockedId) {
        match self.peer(peer_id).conn_state() {
            ConnectedState::Disconnected => {}
            conn_state => panic!(
                "unexpected peer conn_state when requesting to connect to peer from node: {:?}",
                conn_state
            ),
        }
        self.dispatch(
            // Adding peer as potential peer will cause node to connect
            // to it if it doesn't have enough peers.
            PeersAddMultiAction {
                addresses: vec![peer_id.to_ipv4()],
            },
        );
    }

    pub fn set_peer_connected(&mut self, peer_id: MioPeerMockedId) {
        match self.peer(peer_id).conn_state_mut() {
            conn_state @ ConnectedState::IncomingInit => {
                *conn_state = ConnectedState::IncomingConnected;
            }
            conn_state @ ConnectedState::OutgoingAccepted => {
                *conn_state = ConnectedState::OutgoingConnected;
            }
            conn_state => panic!(
                "unexpected peer conn_state when setting peer connected: {:?}",
                conn_state
            ),
        }

        self.dispatch_peer_ready_event(peer_id, false, true, false);
    }

    /// Automatically create and set peer crypto based on stored conn messages.
    /// [Self::set_sent_conn_message] and [Self::set_received_conn_msg]
    /// should be called before this function can be used.
    pub fn auto_set_peer_crypto(
        &mut self,
        peer_id: MioPeerMockedId,
    ) -> Result<&mut Self, HandshakeError> {
        let peer = self.peer(peer_id);
        if peer.crypto().is_some() {
            return Ok(self);
        }
        let sent = peer.sent_conn_msg.clone().unwrap();
        let received = peer.received_conn_msg.clone().unwrap();

        self.set_peer_crypto_with_conn_messages(peer_id, &sent, &received)
    }

    pub fn set_peer_crypto(
        &mut self,
        peer_id: MioPeerMockedId,
        peer_public_key: &PublicKey,
        sent_conn_msg: &BinaryChunk,
        received_conn_msg: &BinaryChunk,
    ) -> Result<&mut Self, HandshakeError> {
        let peer = self.peer(peer_id);

        let incoming = match &peer.conn_state() {
            ConnectedState::IncomingConnected => true,
            ConnectedState::OutgoingConnected => false,
            _ => return Err(HandshakeError::NotConnected),
        };

        *peer.crypto_mut() = Some(
            PeerCrypto::build(
                &peer.identity().secret_key,
                peer_public_key,
                sent_conn_msg,
                received_conn_msg,
                incoming,
            )
            .unwrap(),
        );

        Ok(self)
    }

    /// Bytes must be without chunk's length.
    #[inline]
    pub fn set_peer_crypto_with_bytes(
        &mut self,
        peer_id: MioPeerMockedId,
        peer_public_key: &[u8],
        sent_conn_msg: &[u8],
        received_conn_msg: &[u8],
    ) -> Result<&mut Self, HandshakeError> {
        self.set_peer_crypto(
            peer_id,
            &PublicKey::from_bytes(peer_public_key).unwrap(),
            &BinaryChunk::from_content(sent_conn_msg).unwrap(),
            &BinaryChunk::from_content(received_conn_msg).unwrap(),
        )
    }

    #[inline]
    pub fn set_peer_crypto_with_conn_messages(
        &mut self,
        peer_id: MioPeerMockedId,
        sent_conn_msg: &ConnectionMessage,
        received_conn_msg: &ConnectionMessage,
    ) -> Result<&mut Self, HandshakeError> {
        self.set_peer_crypto_with_bytes(
            peer_id,
            received_conn_msg.public_key(),
            &sent_conn_msg.as_bytes().unwrap(),
            &received_conn_msg.as_bytes().unwrap(),
        )
    }

    /// Exchange connection messages and create crypto to encrypt/decrypt messages.
    pub fn init_crypto_for_peer(
        &mut self,
        peer_id: MioPeerMockedId,
    ) -> Result<&mut Self, HandshakeError> {
        let network_version = self
            .state()
            .config
            .shell_compatibility_version
            .to_network_version();
        let peer = &mut self.peer(peer_id);
        peer.set_read_cond(IOCondition::NoLimit)
            .set_write_cond(IOCondition::NoLimit);

        let sent_conn_msg = ConnectionMessage::try_new(
            peer_id.to_ipv4().port(),
            &peer.identity().public_key,
            &peer.identity().proof_of_work_stamp,
            Nonce::random(),
            network_version,
        )
        .unwrap();

        peer.send_conn_msg(&sent_conn_msg);
        self.dispatch_peer_ready_event(peer_id, true, true, false);

        let received_conn_msg = self.peer(peer_id).read_conn_msg();

        self.set_peer_crypto_with_conn_messages(peer_id, &sent_conn_msg, &received_conn_msg)
    }

    /// Do Handshake between node and fake peer.
    ///
    /// 1. Exchange [ConnectionMessage] unless we already have crypto set.
    ///    Using that set crypto to encrypt/decrypt further messages.
    /// 2. Exchange encrypted [MetadataMessage].
    /// 3. Exchange encrypted [AckMessage].
    pub fn do_handshake(&mut self, peer_id: MioPeerMockedId) -> Result<&mut Self, HandshakeError> {
        if self.peer(peer_id).crypto().is_none() {
            self.init_crypto_for_peer(peer_id)?;
        }
        self.peer(peer_id)
            .set_read_cond(IOCondition::NoLimit)
            .send_meta_msg(&MetadataMessage::new(false, false));

        self.dispatch_peer_ready_event(peer_id, true, true, false);

        let peer = self.peer(peer_id);
        peer.read_meta_msg();

        peer.set_read_cond(IOCondition::NoLimit)
            .send_ack_msg(&AckMessage::Ack);
        self.dispatch_peer_ready_event(peer_id, true, true, false);

        let peer = self.peer(peer_id);
        match peer.read_ack_message() {
            AckMessage::Ack => Ok(self),
            AckMessage::NackV0 => Err(HandshakeError::NackV0),
            AckMessage::Nack(nack_info) => Err(HandshakeError::Nack(nack_info)),
        }
    }
}
