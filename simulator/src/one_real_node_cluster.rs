use std::collections::VecDeque;
use std::fmt::Debug;
use std::io;
use std::time::{Duration, Instant};

use crypto::crypto_box::{CryptoKey, PublicKey};
use crypto::nonce::Nonce;
use tezedge_state::proposer::{
    Event, Manager, Peer, TezedgeProposer, TezedgeProposerConfig,
};
use tezedge_state::{DefaultEffects, PeerAddress, PeerCrypto, TezedgeState};
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryWrite};
use tezos_messages::p2p::encoding::ack::NackInfo;
use tezos_messages::p2p::encoding::prelude::{
    AckMessage, ConnectionMessage, MetadataMessage, NetworkVersion, PeerMessage,
};

use crate::fake_event::{FakeEvent, FakeNetworkEvent, FakeNetworkEventType};
use crate::fake_events::FakeEvents;
use crate::fake_peer::{ConnectedState, FakePeer, FakePeerStream, IOCondition};
use crate::fake_peer_id::FakePeerId;

/// Events with time difference of less than given duration will be
/// grouped together.
pub const EVENT_GROUP_BOUNDARY: Duration = Duration::from_millis(1);

#[derive(Debug)]
pub enum ConnectToNodeError {
    NodeNotListening,
}

#[derive(Debug)]
pub enum HandshakeError {
    NotConnected,
    NackV0,
    Nack(NackInfo),
}

#[derive(Debug, Clone)]
pub struct OneRealNodeManager {
    listening: bool,
    pow_target: f64,
    wait_for_events_timeout: Option<Duration>,

    fake_peers: Vec<Peer<FakePeerStream>>,
    events: VecDeque<FakeEvent>,
    last_event_time: Option<Instant>,
}

impl OneRealNodeManager {
    fn new(pow_target: f64, wait_for_events_timeout: Option<Duration>) -> Self {
        Self {
            listening: false,
            pow_target,
            wait_for_events_timeout,
            fake_peers: vec![],
            events: VecDeque::new(),
            last_event_time: None,
        }
    }

    /// Panics if peer with such id is not found.
    fn get_mut(&mut self, id: FakePeerId) -> &mut FakePeer {
        &mut self.fake_peers[id.index()]
    }

    fn init_new_fake_peer(&mut self) -> FakePeerId {
        let peer_id = FakePeerId::new_unchecked(self.fake_peers.len());
        self.fake_peers.push(Peer::new(
            peer_id.into(),
            FakePeerStream::new(self.pow_target),
        ));
        peer_id
    }

    fn push_event(&mut self, event: FakeEvent) {
        self.events.push_back(event);
    }
}

impl Manager for OneRealNodeManager {
    type Stream = FakePeerStream;
    type NetworkEvent = FakeNetworkEvent;
    type Events = FakeEvents;

    fn start_listening_to_server_events(&mut self) -> io::Result<()> {
        self.listening = true;
        Ok(())
    }
    fn stop_listening_to_server_events(&mut self) {
        self.listening = false;
    }

    fn accept_connection(&mut self, event: &Self::NetworkEvent) -> Option<&mut Peer<Self::Stream>> {
        let peer = self.get_mut(event.from);
        match peer.stream.conn_state_mut() {
            conn_state @ ConnectedState::Disconnected
            | conn_state @ ConnectedState::Outgoing(false) => {
                *conn_state = ConnectedState::Outgoing(true);
                Some(peer)
            }
            _ => None,
        }
    }

    fn wait_for_events(&mut self, events_container: &mut Self::Events, timeout: Option<Duration>) {
        events_container.events.clear();

        let first_event_time = match self.events.get(0) {
            Some(e) => e.time(),
            None => return,
        };

        // simulate tick events.
        if let Some(&last_event_time) = self.last_event_time.as_ref() {
            if let Some(&wait_for_events_timeout) = self.wait_for_events_timeout.as_ref() {
                let time = last_event_time + wait_for_events_timeout;

                if time < first_event_time {
                    if let Ok(_) = events_container.try_push(FakeEvent::Tick(time)) {
                        self.last_event_time = Some(time);
                    }
                    return;
                }
            }
        }

        while let Some(event) = self.events.pop_front() {
            let event_time = event.time();
            if event_time.duration_since(first_event_time) > EVENT_GROUP_BOUNDARY {
                self.events.push_front(event);
                return;
            }
            if let Err(event) = events_container.try_push(event.clone()) {
                self.events.push_front(event);
                return;
            }
            self.last_event_time = Some(event_time);
            match event {
                Event::Network(event) => {
                    self.get_mut(event.from)
                        .stream
                        .set_io_cond_from_event(&event);
                }
                Event::Tick(_) => {}
            }
        }
    }

    fn get_peer(&mut self, address: &PeerAddress) -> Option<&mut Peer<Self::Stream>> {
        Some(self.get_mut(address.into()))
    }

    fn get_peer_or_connect_mut(
        &mut self,
        address: &PeerAddress,
    ) -> io::Result<&mut Peer<Self::Stream>> {
        let peer = self.get_mut(address.into());
        match peer.stream.conn_state_mut() {
            conn_state @ ConnectedState::Disconnected
            | conn_state @ ConnectedState::Incoming(false) => {
                *conn_state = ConnectedState::Incoming(true);
                Ok(peer)
            }
            ConnectedState::Outgoing(true) => Err(io::Error::new(
                io::ErrorKind::Other,
                "unexpected fake peer conn_state: ConnectedState::Outgoing!",
            )),
            conn_state => Err(io::Error::new(
                io::ErrorKind::Other,
                format!("unexpected fake peer conn_state: {:?}!", conn_state),
            )),
        }
    }

    fn get_peer_for_event_mut(
        &mut self,
        event: &Self::NetworkEvent,
    ) -> Option<&mut Peer<Self::Stream>> {
        Some(self.get_mut(event.from))
    }

    fn disconnect_peer(&mut self, peer: &PeerAddress) {
        self.get_mut(peer.into()).stream.disconnect();
    }
}

#[derive(Clone)]
pub struct OneRealNodeCluster {
    time: Instant,
    proposer: TezedgeProposer<FakeEvents, DefaultEffects, OneRealNodeManager>,
}

impl OneRealNodeCluster {
    pub fn new(
        initial_time: Instant,
        proposer_config: TezedgeProposerConfig,
        state: TezedgeState,
    ) -> OneRealNodeCluster {
        let wait_for_events_timeout = proposer_config.wait_for_events_timeout;
        let pow_target = state.config().pow_target;
        let mut proposer = TezedgeProposer::new(
            proposer_config,
            DefaultEffects::default(),
            state,
            FakeEvents::new(),
            OneRealNodeManager::new(pow_target, wait_for_events_timeout),
        );
        // needed to start listening for incoming connections.
        proposer.make_progress();

        Self {
            time: initial_time,
            proposer,
        }
    }

    pub fn proposer(&self) -> &TezedgeProposer<FakeEvents, DefaultEffects, OneRealNodeManager> {
        &self.proposer
    }

    pub fn advance_time(&mut self, by: Duration) -> &mut Self {
        self.time += by;
        self
    }

    pub fn advance_time_ms(&mut self, by: u64) -> &mut Self {
        self.time += Duration::from_millis(by);
        self
    }

    /// Whether events queue is empty.
    pub fn is_done(&self) -> bool {
        self.proposer.manager.events.is_empty()
    }

    pub fn make_progress(&mut self) -> &mut Self {
        self.proposer.make_progress();
        self
    }

    pub fn assert_state(&mut self) -> &mut Self {
        self.proposer.assert_state();
        self
    }

    fn extend_node_potential_peers<I>(&mut self, peers: I)
    where
        I: IntoIterator<Item = FakePeerId>,
        I::IntoIter: Debug,
    {
        self.proposer
            .extend_potential_peers(self.time, peers.into_iter().map(|x| x.into()))
    }

    pub fn init_new_fake_peer(&mut self) -> FakePeerId {
        self.proposer.manager.init_new_fake_peer()
    }

    pub fn get_peer(&mut self, peer_id: FakePeerId) -> &mut FakePeerStream {
        &mut self.proposer.manager.get_mut(peer_id).stream
    }

    pub fn is_peer_connected(&mut self, peer_id: FakePeerId) -> bool {
        self.get_peer(peer_id).is_connected()
    }

    /// Connect to node if not already connected.
    pub fn connect_to_node(&mut self, from: FakePeerId) -> Result<&mut Self, ConnectToNodeError> {
        if self.proposer.manager.listening == false {
            return Err(ConnectToNodeError::NodeNotListening);
        }

        self.proposer
            .manager
            .get_mut(from)
            .stream
            .conn_state_mut()
            .to_outgoing();
        self.proposer.manager.push_event(
            FakeNetworkEvent {
                time: self.time,
                from,
                event_type: FakeNetworkEventType::IncomingConnection,
            }
            .into(),
        );

        Ok(self)
    }

    /// Connect to fake peer from node if maximum number of outgoing
    /// connections isn't reached for node.
    pub fn connect_from_node(&mut self, to: FakePeerId) -> &mut Self {
        self.extend_node_potential_peers(std::iter::once(to));
        self.proposer
            .manager
            .get_mut(to)
            .stream
            .conn_state_mut()
            .to_incoming();

        self
    }

    pub fn disconnect_peer(&mut self, peer_id: FakePeerId) -> &mut Self {
        self.proposer
            .manager
            .events
            .push_back(Event::Network(FakeNetworkEvent {
                time: self.time,
                from: peer_id,
                event_type: FakeNetworkEventType::Disconnected,
            }));

        self
    }

    pub fn add_event(
        &mut self,
        peer_id: FakePeerId,
        event_type: FakeNetworkEventType,
    ) -> &mut Self {
        self.proposer
            .manager
            .events
            .push_back(Event::Network(FakeNetworkEvent {
                time: self.time,
                from: peer_id,
                event_type,
            }));

        self
    }

    pub fn add_readable_event(
        &mut self,
        peer_id: FakePeerId,
        read_limit: Option<usize>,
    ) -> &mut Self {
        self.proposer
            .manager
            .events
            .push_back(Event::Network(FakeNetworkEvent {
                time: self.time,
                from: peer_id,
                event_type: FakeNetworkEventType::BytesReadable(read_limit),
            }));

        self
    }

    pub fn add_writable_event(
        &mut self,
        peer_id: FakePeerId,
        write_limit: Option<usize>,
    ) -> &mut Self {
        self.proposer
            .manager
            .events
            .push_back(Event::Network(FakeNetworkEvent {
                time: self.time,
                from: peer_id,
                event_type: FakeNetworkEventType::BytesWritable(write_limit),
            }));

        self
    }

    pub fn set_sent_conn_message(
        &mut self,
        peer_id: FakePeerId,
        conn_msg: ConnectionMessage,
    ) -> &mut Self {
        self.get_peer(peer_id).set_sent_conn_message(conn_msg);
        self
    }

    pub fn set_received_conn_msg(
        &mut self,
        peer_id: FakePeerId,
        conn_msg: ConnectionMessage,
    ) -> &mut Self {
        self.get_peer(peer_id).set_received_conn_msg(conn_msg);
        self
    }

    /// Automatically create and set peer crypto based on stored conn messages.
    /// [Self::set_sent_conn_message] and [Self::set_received_conn_msg]
    /// should be called before this function can be used.
    pub fn auto_set_peer_crypto(
        &mut self,
        peer_id: FakePeerId,
    ) -> Result<&mut Self, HandshakeError> {
        let peer = self.get_peer(peer_id);
        if peer.crypto().is_some() {
            return Ok(self);
        }
        let sent = peer.sent_conn_msg.take().unwrap();
        let received = peer.received_conn_msg.take().unwrap();

        self.set_peer_crypto_with_conn_messages(peer_id, &sent, &received)
    }

    pub fn set_peer_crypto(
        &mut self,
        peer_id: FakePeerId,
        peer_public_key: &PublicKey,
        sent_conn_msg: &BinaryChunk,
        received_conn_msg: &BinaryChunk,
    ) -> Result<&mut Self, HandshakeError> {
        let peer = self.get_peer(peer_id);

        let incoming = match &peer.conn_state() {
            ConnectedState::Incoming(_) => true,
            ConnectedState::Outgoing(_) => false,
            ConnectedState::Disconnected => return Err(HandshakeError::NotConnected),
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
        peer_id: FakePeerId,
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
        peer_id: FakePeerId,
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
        peer_id: FakePeerId,
    ) -> Result<&mut Self, HandshakeError> {
        let peer = &mut self.get_peer(peer_id);
        peer.set_read_cond(IOCondition::NoLimit)
            .set_write_cond(IOCondition::NoLimit);

        let sent_conn_msg = ConnectionMessage::try_new(
            12345,
            &peer.identity().public_key,
            &peer.identity().proof_of_work_stamp,
            Nonce::random(),
            NetworkVersion::new("TEZOS_MAINNET".to_owned(), 0, 1),
        )
        .unwrap();

        peer.send_conn_msg(&sent_conn_msg);
        self.add_readable_event(peer_id, None)
            .add_writable_event(peer_id, None)
            .make_progress();

        let received_conn_msg = self.get_peer(peer_id).read_conn_msg();

        self.set_peer_crypto_with_conn_messages(peer_id, &sent_conn_msg, &received_conn_msg)
    }

    /// Do Handshake between node and fake peer.
    ///
    /// 1. Exchange [ConnectionMessage] unless we already have crypto set.
    ///    Using that set crypto to encrypt/decrypt further messages.
    /// 2. Exchange encrypted [MetadataMessage].
    /// 3. Exchange encrypted [AckMessage].
    pub fn do_handshake(&mut self, peer_id: FakePeerId) -> Result<&mut Self, HandshakeError> {
        if self.get_peer(peer_id).crypto().is_none() {
            self.init_crypto_for_peer(peer_id)?;
        }
        self.get_peer(peer_id)
            .send_meta_msg(&MetadataMessage::new(false, false));

        self.add_readable_event(peer_id, None)
            .add_writable_event(peer_id, None)
            .make_progress();

        let peer = self.get_peer(peer_id);
        peer.read_meta_msg();

        peer.send_ack_msg(&AckMessage::Ack);
        self.add_readable_event(peer_id, None)
            .add_writable_event(peer_id, None)
            .make_progress();

        let peer = self.get_peer(peer_id);
        match peer.read_ack_message() {
            AckMessage::Ack => Ok(self),
            AckMessage::NackV0 => Err(HandshakeError::NackV0),
            AckMessage::Nack(nack_info) => Err(HandshakeError::Nack(nack_info)),
        }
    }

    pub fn send_bytes_to_node(&mut self, from: FakePeerId, bytes: &[u8]) -> &mut Self {
        self.get_peer(from).send_bytes(bytes);
        self
    }

    pub fn send_message_to_node(&mut self, from: FakePeerId, message: PeerMessage) {
        unimplemented!()
    }

    pub fn read_message_from_node(&mut self, message_for: FakePeerId) -> PeerMessage {
        unimplemented!()
    }

    pub fn add_tick_for_current_time(&mut self) -> &mut Self {
        self.proposer
            .manager
            .events
            .push_back(Event::Tick(self.time));

        self
    }
}

#[cfg(test)]
mod tests {
    use tezedge_state::{sample_tezedge_state, TezedgeConfig};

    use super::*;

    #[test]
    fn test_can_handshake_incoming() {
        let initial_time = Instant::now();
        let mut cluster = OneRealNodeCluster::new(
            initial_time,
            TezedgeProposerConfig {
                wait_for_events_timeout: Some(Duration::from_millis(250)),
                events_limit: 1024,
            },
            sample_tezedge_state::build(
                initial_time,
                TezedgeConfig {
                    port: 9732,
                    disable_mempool: true,
                    private_node: false,
                    disable_quotas: true,
                    disable_blacklist: true,
                    min_connected_peers: 1,
                    max_connected_peers: 100,
                    max_pending_peers: 100,
                    max_potential_peers: 1000,
                    periodic_react_interval: Duration::from_millis(250),
                    reset_quotas_interval: Duration::from_secs(5),
                    peer_blacklist_duration: Duration::from_secs(15 * 60),
                    peer_timeout: Duration::from_secs(8),
                    // use high number to speed up identity generation.
                    pow_target: 1.0,
                },
                &mut DefaultEffects::default(),
            ),
        );

        let peer_id = cluster.init_new_fake_peer();

        cluster
            .connect_to_node(peer_id)
            .unwrap()
            .make_progress()
            .do_handshake(peer_id)
            .unwrap()
            .make_progress();
    }

    #[test]
    fn test_can_handshake_outgoing() {
        let initial_time = Instant::now();
        let mut cluster = OneRealNodeCluster::new(
            initial_time,
            TezedgeProposerConfig {
                wait_for_events_timeout: Some(Duration::from_millis(250)),
                events_limit: 1024,
            },
            sample_tezedge_state::build(
                initial_time,
                TezedgeConfig {
                    port: 9732,
                    disable_mempool: true,
                    private_node: false,
                    disable_quotas: true,
                    disable_blacklist: true,
                    min_connected_peers: 1,
                    max_connected_peers: 100,
                    max_pending_peers: 100,
                    max_potential_peers: 1000,
                    periodic_react_interval: Duration::from_millis(250),
                    reset_quotas_interval: Duration::from_secs(5),
                    peer_blacklist_duration: Duration::from_secs(15 * 60),
                    peer_timeout: Duration::from_secs(8),
                    // use high number to speed up identity generation.
                    pow_target: 1.0,
                },
                &mut DefaultEffects::default(),
            ),
        );

        let peer_id = cluster.init_new_fake_peer();

        cluster
            .connect_from_node(peer_id)
            .make_progress()
            .do_handshake(peer_id)
            .unwrap()
            .make_progress();
    }
}
