// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::fmt::Debug;
use std::io::{self, Read, Write};

use shell_automaton::peer::PeerCrypto;
use shell_automaton::service::mio_service::MioPeer;
use tezos_identity::Identity;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryRead, BinaryWrite};
use tezos_messages::p2p::encoding::prelude::{
    AckMessage, ConnectionMessage, MetadataMessage, PeerMessage, PeerMessageResponse,
};

use super::chunking::{ChunkReadBuffer, EncryptedMessageWriter, MessageReadBuffer};

pub type MioPeerMocked = MioPeer<MioPeerStreamMocked>;

#[derive(Debug, Clone, Copy)]
pub enum ConnectedState {
    /// Disconnected.
    Disconnected,

    /// Incoming connection to the mocked peer from real node initiated.
    IncomingInit,
    /// Incoming connection to the mocked peer from real node established.
    IncomingConnected,

    /// Outgoing connection from the mocked peer to real node initiated.
    OutgoingInit,
    /// Outgoing connection from the mocked peer to real node accepted.
    OutgoingAccepted,
    /// Outgoing connection from the mocked peer to real node established.
    OutgoingConnected,
}

impl ConnectedState {
    pub fn is_connected(&self) -> bool {
        match self {
            Self::Disconnected => false,
            Self::IncomingInit => false,
            Self::IncomingConnected => true,
            Self::OutgoingInit => false,
            Self::OutgoingAccepted => false,
            Self::OutgoingConnected => true,
        }
    }

    pub fn disconnect(&mut self) {
        *self = Self::Disconnected;
    }
}

#[derive(Debug, Clone)]
pub enum IOCondition {
    NoLimit,
    Limit(usize),
    Error(io::ErrorKind),
}

#[derive(Debug, Clone)]
pub struct MioPeerStreamMocked {
    conn_state: ConnectedState,
    identity: Identity,

    pub sent_conn_msg: Option<ConnectionMessage>,
    pub received_conn_msg: Option<ConnectionMessage>,
    crypto: Option<PeerCrypto>,

    read_buf: VecDeque<u8>,
    write_buf: VecDeque<u8>,

    read_cond: IOCondition,
    write_cond: IOCondition,
}

impl MioPeerStreamMocked {
    pub fn new(pow_target: f64) -> Self {
        Self::with_identity(Identity::generate(pow_target).unwrap())
    }

    pub fn with_identity(identity: Identity) -> Self {
        Self {
            conn_state: ConnectedState::Disconnected,
            identity,

            sent_conn_msg: None,
            received_conn_msg: None,
            crypto: None,

            read_buf: VecDeque::new(),
            write_buf: VecDeque::new(),

            read_cond: IOCondition::Limit(0),
            write_cond: IOCondition::Limit(0),
        }
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn read_buf(&self) -> &VecDeque<u8> {
        &self.read_buf
    }

    pub fn write_buf(&self) -> &VecDeque<u8> {
        &self.write_buf
    }

    pub fn is_connected(&self) -> bool {
        self.conn_state.is_connected()
    }

    pub fn conn_state(&self) -> ConnectedState {
        self.conn_state
    }

    pub fn conn_state_mut(&mut self) -> &mut ConnectedState {
        &mut self.conn_state
    }

    pub fn crypto(&self) -> Option<&PeerCrypto> {
        self.crypto.as_ref()
    }

    pub fn crypto_mut(&mut self) -> &mut Option<PeerCrypto> {
        &mut self.crypto
    }

    pub fn disconnect(&mut self) {
        self.conn_state.disconnect()
    }

    pub fn set_read_cond(&mut self, cond: IOCondition) -> &mut Self {
        self.read_cond = cond;
        self
    }

    pub fn set_write_cond(&mut self, cond: IOCondition) -> &mut Self {
        self.write_cond = cond;
        self
    }

    pub fn set_sent_conn_message(&mut self, conn_msg: ConnectionMessage) {
        self.sent_conn_msg = Some(conn_msg);
    }

    pub fn set_received_conn_msg(&mut self, conn_msg: ConnectionMessage) {
        self.received_conn_msg = Some(conn_msg);
    }

    pub fn send_bytes(&mut self, bytes: &[u8]) {
        self.read_buf.extend(bytes);
    }

    /// Send connection message to real node.
    pub fn send_conn_msg(&mut self, conn_msg: &ConnectionMessage) {
        self.sent_conn_msg = Some(conn_msg.clone());
        self.send_bytes(
            BinaryChunk::from_content(&conn_msg.as_bytes().unwrap())
                .unwrap()
                .raw(),
        );
    }

    pub fn read_conn_msg(&mut self) -> ConnectionMessage {
        let mut reader = ChunkReadBuffer::new();
        while !reader.is_finished() {
            reader
                .read_from(&mut VecDequeReadable::from(&mut self.write_buf))
                .unwrap();
        }
        let conn_msg =
            ConnectionMessage::from_bytes(reader.take_if_ready().unwrap().content()).unwrap();
        self.received_conn_msg = Some(conn_msg.clone());
        conn_msg
    }

    pub fn send_meta_msg(&mut self, meta_msg: &MetadataMessage) {
        let crypto = self
            .crypto
            .as_mut()
            .expect("missing PeerCrypto for encryption");
        let mut encrypted_msg_writer = EncryptedMessageWriter::try_new(meta_msg).unwrap();
        encrypted_msg_writer
            .write_to_extendable(&mut self.read_buf, crypto)
            .unwrap();
    }

    pub fn read_meta_msg(&mut self) -> MetadataMessage {
        let crypto = self
            .crypto
            .as_mut()
            .expect("missing PeerCrypto for encryption");
        let mut reader = ChunkReadBuffer::new();
        while !reader.is_finished() {
            reader
                .read_from(&mut VecDequeReadable::from(&mut self.write_buf))
                .unwrap();
        }
        let bytes = crypto
            .decrypt(&reader.take_if_ready().unwrap().content())
            .unwrap();
        MetadataMessage::from_bytes(bytes).unwrap()
    }

    pub fn send_ack_msg(&mut self, ack_msg: &AckMessage) {
        let crypto = self
            .crypto
            .as_mut()
            .expect("missing PeerCrypto for encryption");
        let mut encrypted_msg_writer = EncryptedMessageWriter::try_new(ack_msg).unwrap();
        encrypted_msg_writer
            .write_to_extendable(&mut self.read_buf, crypto)
            .unwrap();
    }

    pub fn read_ack_message(&mut self) -> AckMessage {
        let crypto = self
            .crypto
            .as_mut()
            .expect("missing PeerCrypto for encryption");
        let mut reader = ChunkReadBuffer::new();
        while !reader.is_finished() {
            reader
                .read_from(&mut VecDequeReadable::from(&mut self.write_buf))
                .unwrap();
        }
        let bytes = crypto
            .decrypt(&reader.take_if_ready().unwrap().content())
            .unwrap();
        AckMessage::from_bytes(bytes).unwrap()
    }

    pub fn send_peer_message(&mut self, peer_message: PeerMessage) {
        let crypto = self
            .crypto
            .as_mut()
            .expect("missing PeerCrypto for encryption");
        let resp = PeerMessageResponse::from(peer_message);
        let mut encrypted_msg_writer = EncryptedMessageWriter::try_new(&resp).unwrap();
        encrypted_msg_writer
            .write_to_extendable(&mut self.read_buf, crypto)
            .unwrap();
    }

    pub fn read_peer_message(&mut self) -> Option<PeerMessage> {
        let crypto = self
            .crypto
            .as_mut()
            .expect("missing PeerCrypto for encryption");
        if self.write_buf.len() == 0 {
            return None;
        }
        let mut reader = MessageReadBuffer::new();
        Some(
            reader
                .read_from(&mut VecDequeReadable::from(&mut self.write_buf), crypto)
                .unwrap()
                .message,
        )
    }
}

impl Read for MioPeerStreamMocked {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        assert_ne!(
            buf.len(),
            0,
            "empty(len = 0) buffer shouldn't be passed to read method"
        );
        let len = match &mut self.read_cond {
            IOCondition::NoLimit => buf.len(),
            IOCondition::Limit(limit) => {
                let min = buf.len().min(*limit);
                *limit -= min;
                min
            }
            IOCondition::Error(err_kind) => {
                return Err(io::Error::new(*err_kind, "manual error"));
            }
        };
        // eprintln!("read {}. limit: {:?}", len, &self.read_limit);

        let len = VecDequeReadable::from(&mut self.read_buf).read(&mut buf[..len])?;

        if self.read_buf.len() == 0 {
            self.set_read_cond(IOCondition::Limit(0));
        }

        Ok(len)
    }
}

impl Write for MioPeerStreamMocked {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let len = match &mut self.write_cond {
            IOCondition::NoLimit => buf.len(),
            IOCondition::Limit(limit) => {
                let min = buf.len().min(*limit);
                *limit -= min;
                min
            }
            IOCondition::Error(err_kind) => {
                return Err(io::Error::new(*err_kind, "manual error"));
            }
        };

        self.write_buf.extend(&buf[..len]);

        // TODO: somehow reset IOCondition::NoLimit to IOCondition::Limit(0),
        // maybe after message has been read.

        Ok(len)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct VecDequeReadable<'a> {
    v: &'a mut VecDeque<u8>,
}

impl<'a> Read for VecDequeReadable<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = self.v.len().min(buf.len());
        for (i, v) in self.v.drain(..len).enumerate() {
            buf[i] = v;
        }
        Ok(len)
    }
}

impl<'a> From<&'a mut VecDeque<u8>> for VecDequeReadable<'a> {
    fn from(v: &'a mut VecDeque<u8>) -> Self {
        Self { v }
    }
}
