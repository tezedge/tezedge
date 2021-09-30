use std::io;

use serde::{Deserialize, Serialize};

use crypto::{crypto_box::PrecomputedKey, nonce::Nonce, CryptoError};
use tezos_messages::p2p::binary_message::{BinaryChunk, BinaryChunkError};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerConnectionChunkWriteState {
    Init,
    Pending,
    Ready,
    Error(PeerChunkWriteError),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WriteCrypto {
    pub precomputed_key: PrecomputedKey,
    pub local_nonce: Nonce,
}

impl WriteCrypto {
    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.precomputed_key.encrypt(data, &self.local_nonce)
    }

    pub fn increment_nonce(&mut self) {
        self.local_nonce = self.local_nonce.increment()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerChunkWriteError {
    IO(String),
    Crypto(String),
    Chunk(String),
}

impl From<io::Error> for PeerChunkWriteError {
    fn from(error: io::Error) -> Self {
        Self::IO(error.to_string())
    }
}

impl From<CryptoError> for PeerChunkWriteError {
    fn from(error: CryptoError) -> Self {
        Self::Crypto(error.to_string())
    }
}

impl From<BinaryChunkError> for PeerChunkWriteError {
    fn from(error: BinaryChunkError) -> Self {
        Self::Crypto(error.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkWrite {
    pub crypto: WriteCrypto,
    pub state: PeerChunkWriteState,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerChunkWriteState {
    Init,
    UnencryptedContent { content: Vec<u8> },
    EncryptedContent { content: Vec<u8> },
    Pending { chunk: BinaryChunk, written: usize },
    Ready,
    Error { error: PeerChunkWriteError },
}
