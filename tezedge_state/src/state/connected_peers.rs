use crypto::crypto_box::PublicKey;
use getset::{CopyGetters, Getters};
use slog::Logger;
use std::collections::{HashMap, VecDeque};
use std::io::{Read, Write};
use std::time::{Duration, Instant};

use crate::chunking::{
    EncryptedMessageWriter, MessageReadBuffer, ReadMessageError, WriteMessageError,
};
use crate::peer_address::PeerListenerAddress;
use crate::state::pending_peers::HandshakeResult;
use crate::state::ThrottleQuota;
use crate::{PeerAddress, PeerCrypto, Port};
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::NetworkVersion;

#[derive(Getters, CopyGetters, Debug, Clone)]
pub struct ConnectedPeer {
    // #[get = "pub"]
    pub address: PeerAddress,

    // #[get = "pub"]
    pub port: Port,

    // #[get = "pub"]
    pub version: NetworkVersion,

    // #[get = "pub"]
    pub public_key: PublicKey,

    // #[get = "pub"]
    pub crypto: PeerCrypto,

    // #[get_copy = "pub"]
    pub disable_mempool: bool,

    // #[get_copy = "pub"]
    pub private_node: bool,

    // #[get_copy = "pub"]
    pub connected_since: Instant,

    read_buf: MessageReadBuffer,

    cur_send_message: Option<EncryptedMessageWriter>,
    send_message_queue: VecDeque<PeerMessage>,

    quota: ThrottleQuota,
}

impl ConnectedPeer {
    pub fn listener_port(&self) -> Port {
        self.port
    }

    pub fn listener_address(&self) -> PeerListenerAddress {
        PeerListenerAddress::new(self.address.ip(), self.port)
    }

    pub fn read_message_from<R: Read>(
        &mut self,
        reader: &mut R,
    ) -> Result<PeerMessage, ReadMessageError> {
        let msg = self.read_buf.read_from(reader, &mut self.crypto)?;
        if self.quota.can_receive(&msg).is_ok() {
            Ok(msg)
        } else {
            Err(ReadMessageError::QuotaReached)
        }
    }

    /// Enqueue message to be sent to the peer.
    pub fn enqueue_send_message(&mut self, message: PeerMessage) {
        if let Ok(_) = self.quota.can_send(&message) {
            self.send_message_queue.push_back(message);
        }
    }

    /// Write any enqueued messages to the given writer.
    pub fn write_to<W: Write>(&mut self, writer: &mut W) -> Result<(), WriteMessageError> {
        if let Some(message_writer) = self.cur_send_message.as_mut() {
            message_writer.write_to(writer, &mut self.crypto)?;
            self.cur_send_message = None;
            Ok(())
        } else if let Some(message) = self.send_message_queue.pop_front() {
            self.cur_send_message = Some(EncryptedMessageWriter::try_new(
                &PeerMessageResponse::from(message),
            )?);
            self.write_to(writer)
        } else {
            Err(WriteMessageError::Empty)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConnectedPeers {
    log: Logger,
    last_quota_reset: Option<Instant>,
    quota_reset_interval: Duration,
    peers: HashMap<PeerAddress, ConnectedPeer>,
}

impl ConnectedPeers {
    #[inline]
    pub fn new(
        log: Logger,
        capacity: Option<usize>,
        quota_reset_interval: Duration,
    ) -> Self {
        let peers = if let Some(capacity) = capacity {
            HashMap::with_capacity(capacity)
        } else {
            HashMap::new()
        };
        Self {
            log,
            last_quota_reset: None,
            quota_reset_interval,
            peers,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    // fn find_index(&self, address: &PeerAddress) -> Option<usize> {
    //     // TODO: use token instead of address.
    //     self.peers.iter()
    //         .find(|(_, x)| &x.address == address)
    //         .map(|(index, _)| index)
    // }

    #[inline]
    pub fn contains_address(&self, address: &PeerAddress) -> bool {
        // self.find_index(address).is_some()
        self.peers.contains_key(address)
    }

    #[inline]
    pub fn get(&self, id: &PeerAddress) -> Option<&ConnectedPeer> {
        // if let Some(index) = self.find_index(id) {
        //     self.peers.get(index)
        // } else {
        //     None
        // }
        self.peers.get(id)
    }

    #[inline]
    pub fn get_mut(&mut self, id: &PeerAddress) -> Option<&mut ConnectedPeer> {
        // if let Some(index) = self.find_index(id) {
        //     self.peers.get_mut(index)
        // } else {
        //     None
        // }
        self.peers.get_mut(id)
    }

    // #[inline]
    // pub(crate) fn insert(&mut self, peer: ConnectedPeer) -> usize {
    //     self.peers.insert(peer)
    // }

    #[inline]
    pub(crate) fn remove(&mut self, id: &PeerAddress) -> Option<ConnectedPeer> {
        // self.find_index(id)
        //     .map(|index| self.peers.remove(index))
        self.peers.remove(id)
    }

    #[inline]
    // pub fn iter(&self) -> slab::Iter<ConnectedPeer> {
    pub fn iter(&self) -> impl Iterator<Item = &ConnectedPeer> {
        self.peers.iter().map(|(_, peer)| peer)
    }

    #[inline]
    // pub fn iter_mut(&mut self) -> slab::IterMut<ConnectedPeer> {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ConnectedPeer> {
        self.peers.iter_mut().map(|(_, peer)| peer)
    }

    pub(crate) fn set_peer_connected(
        &mut self,
        at: Instant,
        peer_address: PeerAddress,
        result: HandshakeResult,
    ) -> &mut ConnectedPeer {
        let log = &self.log;
        // self.peers.vacant_entry().insert(ConnectedPeer {
        self.peers
            .entry(peer_address)
            .or_insert_with(|| ConnectedPeer {
                connected_since: at,
                address: peer_address,

                port: result.port,
                version: result.compatible_version,
                public_key: result.public_key,
                crypto: result.crypto,
                disable_mempool: result.disable_mempool,
                private_node: result.private_node,

                read_buf: MessageReadBuffer::new(),
                cur_send_message: None,
                send_message_queue: VecDeque::new(),

                quota: ThrottleQuota::new(log.clone()),
            })
    }

    pub(crate) fn periodic_react(&mut self, at: Instant) {
        let last_quota_reset = *self.last_quota_reset.get_or_insert(at);
        if at.duration_since(last_quota_reset) >= self.quota_reset_interval {
            for (_, peer) in self.peers.iter_mut() {
                peer.quota.reset_all();
            }
        }
    }
}
