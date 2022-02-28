// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::io;

use crypto::{crypto_box::PrecomputedKey, nonce::Nonce, CryptoError};

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerChunkReadError {
    /// Received chunk size = 0.
    ZeroSize,
    IO(String),
    Crypto(String),
}

impl From<io::Error> for PeerChunkReadError {
    fn from(error: io::Error) -> Self {
        Self::IO(error.to_string())
    }
}

impl From<CryptoError> for PeerChunkReadError {
    fn from(error: CryptoError) -> Self {
        Self::Crypto(error.to_string())
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReadCrypto {
    pub precomputed_key: PrecomputedKey,
    pub remote_nonce: Nonce,
}

impl ReadCrypto {
    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
        self.precomputed_key.decrypt(data, &self.remote_nonce)
    }

    pub fn increment_nonce(&mut self) {
        self.remote_nonce = self.remote_nonce.increment()
    }
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerChunkRead {
    pub crypto: ReadCrypto,
    pub state: PeerChunkReadState,
}

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone, strum_macros::AsRefStr)]
pub enum PeerChunkReadState {
    Init,
    PendingSize { buffer: Vec<u8> },
    PendingBody { buffer: Vec<u8>, size: usize },
    EncryptedReady { chunk_encrypted: Vec<u8> },
    Ready { chunk: Vec<u8> },
    Error { error: PeerChunkReadError },
}
