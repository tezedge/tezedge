use crypto::blake2b::Blake2bError;
use crypto::nonce::Nonce;
use crypto::CryptoError;
use enum_kinds::EnumKind;
use serde::{Deserialize, Serialize};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::binary_writer::BinaryWriterError;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::prelude::{AckMessage, MetadataMessage};

use crate::peer::binary_message::read::PeerBinaryMessageReadState;
use crate::peer::binary_message::write::PeerBinaryMessageWriteState;
use crate::peer::chunk::read::PeerChunkReadState;
use crate::peer::chunk::write::PeerChunkWriteState;
use crate::peer::{PeerCrypto, PeerToken};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerHandshakingError {
    Chunk(String),
    Crypto(String),
    Encoding(String),
    Decoding(String),
    Blake2b(String),
    Timeout(PeerHandshakingPhase),
}

impl From<CryptoError> for PeerHandshakingError {
    fn from(error: CryptoError) -> Self {
        Self::Crypto(error.to_string())
    }
}

impl From<BinaryWriterError> for PeerHandshakingError {
    fn from(error: BinaryWriterError) -> Self {
        Self::Encoding(error.to_string())
    }
}

impl From<BinaryReaderError> for PeerHandshakingError {
    fn from(error: BinaryReaderError) -> Self {
        Self::Decoding(error.to_string())
    }
}

impl From<BinaryChunkError> for PeerHandshakingError {
    fn from(error: BinaryChunkError) -> Self {
        Self::Chunk(error.to_string())
    }
}

impl From<Blake2bError> for PeerHandshakingError {
    fn from(error: Blake2bError) -> Self {
        Self::Blake2b(error.to_string())
    }
}

#[derive(EnumKind, Serialize, Deserialize, Debug, Clone)]
#[enum_kind(PeerHandshakingPhase, derive(Serialize, Deserialize))]
pub enum PeerHandshakingStatus {
    Init {
        time: u64,
    },
    ConnectionMessageInit {
        time: u64,
        message: ConnectionMessage,
    },
    ConnectionMessageEncoded {
        time: u64,
        binary_message: Vec<u8>,
    },
    ConnectionMessageWritePending {
        time: u64,
        local_chunk: BinaryChunk,
        chunk_state: PeerChunkWriteState,
    },
    ConnectionMessageReadPending {
        time: u64,
        /// Binary chunk for local connection message, used for setting up encryption.
        local_chunk: BinaryChunk,
        chunk_state: PeerChunkReadState,
    },
    /// Connection message exchange complete.
    ConnectionMessageReady {
        time: u64,
        remote_message: ConnectionMessage,
        local_chunk: BinaryChunk,
        remote_chunk: BinaryChunk,
    },

    EncryptionReady {
        time: u64,
        crypto: PeerCrypto,
        remote_connection_message: ConnectionMessage,
    },

    MetadataMessageInit {
        time: u64,
        message: MetadataMessage,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageEncoded {
        time: u64,
        binary_message: Vec<u8>,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageWritePending {
        time: u64,
        binary_message: Vec<u8>,
        binary_message_state: PeerBinaryMessageWriteState,

        /// Encryption data.
        remote_nonce: Nonce,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageReadPending {
        time: u64,
        binary_message_state: PeerBinaryMessageReadState,

        local_nonce: Nonce,
        /// Encryption data.

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageReady {
        time: u64,
        remote_message: MetadataMessage,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    //
    AckMessageInit {
        time: u64,
        message: AckMessage,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageEncoded {
        time: u64,
        binary_message: Vec<u8>,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageWritePending {
        time: u64,
        binary_message: Vec<u8>,
        binary_message_state: PeerBinaryMessageWriteState,

        /// Encryption data.
        remote_nonce: Nonce,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageReadPending {
        time: u64,
        binary_message_state: PeerBinaryMessageReadState,

        /// Encryption data.
        local_nonce: Nonce,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageReady {
        time: u64,
        remote_message: AckMessage,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
        // remote ack message has no data
    }, // TODO Nacked, Blacklisted, ...?
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerHandshaking {
    pub token: PeerToken,
    pub status: PeerHandshakingStatus,
    pub incoming: bool,
    /// We are handshaking with the peer since this time.
    pub since: u64,
}
