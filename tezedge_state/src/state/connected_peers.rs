// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::crypto_box::PublicKey;
use slog::Logger;
use std::collections::{BTreeMap, VecDeque};
use std::io::{Read, Write};
use std::time::{Duration, SystemTime};

use crate::chunking::{
    EncryptedMessageWriter, MessageReadBuffer, ReadMessageError, WriteMessageError,
};
use crate::peer_address::PeerListenerAddress;
use crate::state::pending_peers::HandshakeResult;
use crate::state::ThrottleQuota;
use crate::{PeerAddress, PeerCrypto, Port};
use tezos_messages::p2p::encoding::peer::{PeerMessage, PeerMessageResponse};
use tezos_messages::p2p::encoding::prelude::NetworkVersion;
use tezos_messages::Head;

/// Peer who have undergone handshake.
#[derive(Debug, Clone)]
pub struct ConnectedPeer {
    pub address: PeerAddress,
    pub port: Port,
    pub version: NetworkVersion,
    pub public_key: PublicKey,
    pub crypto: PeerCrypto,
    pub disable_mempool: bool,
    pub private_node: bool,
    pub connected_since: SystemTime,
    read_buf: MessageReadBuffer,
    cur_send_message: Option<EncryptedMessageWriter>,
    send_message_queue: VecDeque<PeerMessage>,
    quota: ThrottleQuota,

    // TOOD: use here Rc?
    /// Peer's last known current head
    current_head: Option<Head>,
    /// Peer's last current head update (can be used to detect stalled peers)
    current_head_update_last: Option<SystemTime>,
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
    ) -> Result<PeerMessageResponse, ReadMessageError> {
        let msg = self.read_buf.read_from(reader, &mut self.crypto)?;
        if self.quota.can_receive(msg.message()) {
            Ok(msg)
        } else {
            Err(ReadMessageError::QuotaReached)
        }
    }

    /// Enqueue message to be sent to the peer.
    pub fn enqueue_send_message(&mut self, message: PeerMessage) {
        if self.quota.can_send(&message) {
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

    pub(crate) fn update_current_head(&mut self, head: Head) {
        self.current_head = Some(head);
        // TODO: how to set time?
        // self.current_head_update_last = Some(SystemTime::now());
    }
}

/// Peers who have undergone handshake.
#[derive(Debug, Clone)]
pub struct ConnectedPeers {
    log: Logger,
    last_quota_reset: Option<SystemTime>,
    quota_reset_interval: Duration,
    peers: BTreeMap<PeerAddress, ConnectedPeer>,
}

impl ConnectedPeers {
    #[inline]
    pub fn new(log: Logger, capacity: Option<usize>, quota_reset_interval: Duration) -> Self {
        let peers = if let Some(_) = capacity {
            // BTreeMap doesn't have `with_capcity` method.
            BTreeMap::new()
        } else {
            BTreeMap::new()
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

    #[inline]
    pub fn contains_address(&self, address: &PeerAddress) -> bool {
        self.peers.contains_key(address)
    }

    #[inline]
    pub fn get(&self, id: &PeerAddress) -> Option<&ConnectedPeer> {
        self.peers.get(id)
    }

    #[inline]
    pub fn get_mut(&mut self, id: &PeerAddress) -> Option<&mut ConnectedPeer> {
        self.peers.get_mut(id)
    }

    #[inline]
    pub(crate) fn remove(&mut self, id: &PeerAddress) -> Option<ConnectedPeer> {
        self.peers.remove(id)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &ConnectedPeer> {
        self.peers.iter().map(|(_, peer)| peer)
    }

    #[inline]
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut ConnectedPeer> {
        self.peers.iter_mut().map(|(_, peer)| peer)
    }

    pub(crate) fn set_peer_connected(
        &mut self,
        at: SystemTime,
        peer_address: PeerAddress,
        result: HandshakeResult,
    ) -> &mut ConnectedPeer {
        let log = &self.log;
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

                current_head: None,
                current_head_update_last: None,
            })
    }

    pub(crate) fn periodic_react(&mut self, at: SystemTime) {
        let last_quota_reset = *self.last_quota_reset.get_or_insert(at);
        let quota_reset_interval_passed = at
            .duration_since(last_quota_reset)
            .map(|passed| passed >= self.quota_reset_interval)
            .unwrap_or(false);
        if quota_reset_interval_passed {
            for (_, peer) in self.peers.iter_mut() {
                peer.quota.reset_all();
            }
        }
    }
}
