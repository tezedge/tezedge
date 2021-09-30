use crypto::blake2b::Blake2bError;
use crypto::crypto_box::PrecomputedKey;
use crypto::nonce::Nonce;
use crypto::CryptoError;
use serde::{Deserialize, Serialize};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::binary_writer::BinaryWriterError;
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError};
use tezos_messages::p2p::encoding::connection::ConnectionMessage;
use tezos_messages::p2p::encoding::prelude::{AckMessage, MetadataMessage};

use crate::peer::binary_message::read::peer_binary_message_read_state::PeerBinaryMessageReadState;
use crate::peer::binary_message::write::peer_binary_message_write_state::PeerBinaryMessageWriteState;
use crate::peer::chunk::read::peer_chunk_read_state::{PeerChunkReadState, ReadCrypto};
use crate::peer::chunk::write::peer_chunk_write_state::{PeerChunkWriteState, WriteCrypto};
use crate::peer::PeerToken;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerHandshakingError {
    Chunk(String),
    Crypto(String),
    Encoding(String),
    Decoding(String),
    Blake2b(String),
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerHandshakingStatus {
    Init,
    ConnectionMessageInit {
        message: ConnectionMessage,
    },
    ConnectionMessageEncoded {
        binary_message: Vec<u8>,
    },
    ConnectionMessageWritePending {
        local_chunk: BinaryChunk,
        chunk_state: PeerChunkWriteState,
    },
    ConnectionMessageReadPending {
        /// Binary chunk for local connection message, used for setting up encryption.
        local_chunk: BinaryChunk,
        chunk_state: PeerChunkReadState,
    },
    /// Connection message exchange complete.
    ConnectionMessageReady {
        remote_message: ConnectionMessage,
        local_chunk: BinaryChunk,
        remote_chunk: BinaryChunk,
    },

    EncryptionReady {
        crypto: PeerCrypto,
        remote_connection_message: ConnectionMessage,
    },

    MetadataMessageInit {
        message: MetadataMessage,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageEncoded {
        binary_message: Vec<u8>,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageWritePending {
        binary_message: Vec<u8>,
        binary_message_state: PeerBinaryMessageWriteState,

        /// Encryption data.
        remote_nonce: Nonce,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageReadPending {
        binary_message_state: PeerBinaryMessageReadState,

        local_nonce: Nonce,
        /// Encryption data.

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    MetadataMessageReady {
        remote_message: MetadataMessage,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
    },
    //
    AckMessageInit {
        message: AckMessage,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageEncoded {
        binary_message: Vec<u8>,

        /// Encryption data.
        crypto: PeerCrypto,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageWritePending {
        binary_message: Vec<u8>,
        binary_message_state: PeerBinaryMessageWriteState,

        /// Encryption data.
        remote_nonce: Nonce,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageReadPending {
        binary_message_state: PeerBinaryMessageReadState,

        /// Encryption data.
        local_nonce: Nonce,

        /// Data collected from previous stages.
        remote_connection_message: ConnectionMessage,
        remote_metadata_message: MetadataMessage,
    },
    AckMessageReady {
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerCrypto {
    pub local_nonce: Nonce,
    pub remote_nonce: Nonce,
    pub precomputed_key: PrecomputedKey,
}

impl PeerCrypto {
    pub fn split_for_reading(self) -> (ReadCrypto, Nonce) {
        (
            ReadCrypto {
                remote_nonce: self.remote_nonce,
                precomputed_key: self.precomputed_key,
            },
            self.local_nonce,
        )
    }

    pub fn unsplit_after_reading(read_crypto: ReadCrypto, local_nonce: Nonce) -> Self {
        Self {
            local_nonce,
            remote_nonce: read_crypto.remote_nonce,
            precomputed_key: read_crypto.precomputed_key,
        }
    }

    pub fn split_for_writing(self) -> (WriteCrypto, Nonce) {
        (
            WriteCrypto {
                local_nonce: self.local_nonce,
                precomputed_key: self.precomputed_key,
            },
            self.remote_nonce,
        )
    }

    pub fn unsplit_after_writing(write_crypto: WriteCrypto, remote_nonce: Nonce) -> Self {
        Self {
            local_nonce: write_crypto.local_nonce,
            remote_nonce,
            precomputed_key: write_crypto.precomputed_key,
        }
    }

    pub fn split(self) -> (ReadCrypto, WriteCrypto) {
        (
            ReadCrypto {
                remote_nonce: self.remote_nonce,
                precomputed_key: self.precomputed_key.clone(),
            },
            WriteCrypto {
                local_nonce: self.local_nonce,
                precomputed_key: self.precomputed_key,
            },
        )
    }
}
