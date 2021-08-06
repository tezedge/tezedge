use std::collections::VecDeque;
use std::fmt::Debug;
use std::io::{self, Read, Write};

use tezedge_state::chunking::{ChunkReadBuffer, EncryptedMessageWriter, MessageReadBuffer};
use tezedge_state::proposer::Peer;
use tezedge_state::PeerCrypto;
use tezos_identity::Identity;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryRead, BinaryWrite};
use tezos_messages::p2p::encoding::prelude::{
    AckMessage, ConnectionMessage, MetadataMessage, PeerMessage, PeerMessageResponse,
};

use crate::fake_event::{FakeNetworkEvent, FakeNetworkEventType};

pub type FakePeer = Peer<FakePeerStream>;

#[derive(Debug, Clone, Copy)]
pub enum ConnectedState {
    /// Disconnected.
    Disconnected,
    /// Incoming connection to the fake peer from real node.
    Incoming(bool),
    /// Outgoing connection from the fake peer to real node.
    Outgoing(bool),
}

impl ConnectedState {
    pub fn is_connected(&self) -> bool {
        match self {
            Self::Disconnected => false,
            Self::Incoming(is_connected) => *is_connected,
            Self::Outgoing(is_connected) => *is_connected,
        }
    }

    pub fn disconnect(&mut self) {
        *self = Self::Disconnected;
    }

    pub fn to_incoming(&mut self) {
        match self {
            Self::Incoming(_) => {}
            Self::Disconnected => *self = Self::Incoming(false),
            Self::Outgoing(_) => {}
        }
    }

    pub fn to_outgoing(&mut self) {
        match self {
            Self::Outgoing(_) => {}
            Self::Disconnected => *self = Self::Outgoing(false),
            Self::Incoming(_) => {}
        }
    }
}

#[derive(Debug, Clone)]
pub enum IOCondition {
    NoLimit,
    Limit(usize),
    Error(io::ErrorKind),
}

#[derive(Debug, Clone)]
pub struct FakePeerStream {
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

impl FakePeerStream {
    pub fn new(pow_target: f64) -> Self {
        Self {
            conn_state: ConnectedState::Disconnected,
            identity: Identity::generate(pow_target).unwrap(),

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

    pub fn set_io_cond_from_event(&mut self, event: &FakeNetworkEvent) {
        match &event.event_type {
            FakeNetworkEventType::BytesReadable(limit) => {
                self.read_cond = match limit {
                    Some(limit) => IOCondition::Limit(*limit),
                    None => IOCondition::NoLimit,
                };
            }
            FakeNetworkEventType::BytesWritable(limit) => {
                self.write_cond = match limit {
                    Some(limit) => IOCondition::Limit(*limit),
                    None => IOCondition::NoLimit,
                };
            }
            FakeNetworkEventType::BytesReadableError(err_kind) => {
                self.read_cond = IOCondition::Error(*err_kind);
            }
            FakeNetworkEventType::BytesWritableError(err_kind) => {
                self.write_cond = IOCondition::Error(*err_kind);
            }
            _ => {}
        }
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
        ConnectionMessage::from_bytes(reader.take_if_ready().unwrap().content()).unwrap()
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

impl Read for FakePeerStream {
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

        VecDequeReadable::from(&mut self.read_buf).read(&mut buf[..len])
    }
}

impl Write for FakePeerStream {
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
        // eprintln!("write {}. limit: {:?}", len, &self.write_limit);

        self.write_buf.extend(&buf[..len]);

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
