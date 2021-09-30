use serde::{Deserialize, Serialize};
use std::io;

use crate::tmp::chunking::ChunkReadBuffer;
use crypto::crypto_box::PublicKey;
use tezos_messages::p2p::binary_message::BinaryChunk;
use tezos_messages::p2p::encoding::ack::NackMotive;
use tezos_messages::p2p::encoding::prelude::{AckMessage, MetadataMessage, NetworkVersion};

use crate::io_error_kind::IOErrorKind;
use crate::peer::{PeerCrypto, PeerToken};
use crate::Port;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageWriteState {
    Idle,
    Pending { written: usize },
    Success,
    // TODO: use custom error instead.
    Error { error: IOErrorKind },
}

impl MessageWriteState {
    #[inline(always)]
    pub fn is_idle(&self) -> bool {
        matches!(self, Self::Idle)
    }

    #[inline(always)]
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum MessageReadState<M> {
    Idle,
    Pending { buffer: ChunkReadBuffer },
    Success { message: M },
    // TODO: use custom error instead.
    Error { error: IOErrorKind },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConnectionMessageDataReceived {
    pub port: Port,
    pub compatible_version: Option<NetworkVersion>,
    pub public_key: PublicKey,
    pub encoded: BinaryChunk,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerHandshakingStatus {
    Init,
    ConnectionMessageWrite {
        /// Encoded `ConnectionMessage` to be sent.
        conn_msg: BinaryChunk,
        status: MessageWriteState,
    },
    ConnectionMessageRead {
        conn_msg_written: BinaryChunk,
        status: MessageReadState<ConnectionMessageDataReceived>,
    },
    MetadataMessageWrite {
        /// Encoded `MetadataMessage` to be sent.
        meta_msg: BinaryChunk,
        status: MessageWriteState,

        // Accumulated data from previous messages.
        port: Port,
        compatible_version: Option<NetworkVersion>,
        public_key: PublicKey,
        crypto: PeerCrypto,
    },
    MetadataMessageRead {
        status: MessageReadState<MetadataMessage>,

        // Accumulated data from previous messages.
        port: Port,
        compatible_version: Option<NetworkVersion>,
        public_key: PublicKey,
        crypto: PeerCrypto,
    },
    AckMessageWrite {
        /// Encoded `MetadataMessage` to be sent.
        ack_msg: BinaryChunk,
        status: MessageWriteState,

        // Accumulated data from previous messages.
        port: Port,
        compatible_version: Option<NetworkVersion>,
        public_key: PublicKey,
        disable_mempool: bool,
        private_node: bool,
        crypto: PeerCrypto,
    },
    AckMessageRead {
        status: MessageReadState<AckMessage>,

        // Accumulated data from previous messages.
        port: Port,
        compatible_version: Option<NetworkVersion>,
        public_key: PublicKey,
        disable_mempool: bool,
        private_node: bool,
        crypto: PeerCrypto,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshaking {
    pub token: PeerToken,
    pub status: PeerHandshakingStatus,
    pub incoming: bool,
}
