// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::blake2b::Blake2bError;
use crypto::crypto_box::{PrecomputedKey, PublicKey, SecretKey};
use crypto::nonce::{generate_nonces, Nonce, NoncePair};
use crypto::CryptoError;
use tezos_messages::p2p::binary_message::BinaryChunk;

use super::chunk::read::ReadCrypto;
use super::chunk::write::WriteCrypto;

/// PeerCrypto is responsible for encrypting/decrypting messages and
/// managing nonces.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerCrypto {
    /// Precomputed key is created from merge of peer public key and our secret key.
    /// It's used to speedup of crypto operations.
    precomputed_key: PrecomputedKey,
    /// Nonce used to encrypt outgoing messages
    nonce_pair: NoncePair,
}

impl PeerCrypto {
    #[inline]
    pub fn new(precomputed_key: PrecomputedKey, nonce_pair: NoncePair) -> Self {
        Self {
            precomputed_key,
            nonce_pair,
        }
    }

    pub fn build(
        node_secret_key: &SecretKey,
        peer_public_key: &PublicKey,
        sent_conn_msg: &BinaryChunk,
        received_conn_msg: &BinaryChunk,
        incoming: bool,
    ) -> Result<Self, Blake2bError> {
        let nonce_pair = generate_nonces(&sent_conn_msg.raw(), &received_conn_msg.raw(), incoming)?;

        let key = PrecomputedKey::precompute(peer_public_key, node_secret_key);

        Ok(PeerCrypto::new(key, nonce_pair))
    }

    #[inline]
    fn local_nonce_fetch_increment(&mut self) -> Nonce {
        let nonce = self.nonce_pair.local.increment();
        std::mem::replace(&mut self.nonce_pair.local, nonce)
    }

    #[inline]
    fn remote_nonce_fetch_increment(&mut self) -> Nonce {
        let nonce = self.nonce_pair.remote.increment();
        std::mem::replace(&mut self.nonce_pair.remote, nonce)
    }

    #[inline]
    pub fn local_nonce(&self) -> Nonce {
        self.nonce_pair.local.clone()
    }

    #[inline]
    pub fn remote_nonce(&self) -> Nonce {
        self.nonce_pair.remote.clone()
    }

    /// Increments local nonce and encrypts the message.
    #[inline]
    pub fn encrypt<T: AsRef<[u8]>>(&mut self, data: &T) -> Result<Vec<u8>, CryptoError> {
        let nonce = self.local_nonce_fetch_increment();
        self.precomputed_key.encrypt(data.as_ref(), &nonce)
    }

    /// Increments remote nonce and encrypts the message.
    #[inline]
    pub fn decrypt<T: AsRef<[u8]>>(&mut self, data: &T) -> Result<Vec<u8>, CryptoError> {
        let nonce = self.remote_nonce_fetch_increment();
        self.precomputed_key.decrypt(data.as_ref(), &nonce)
    }

    pub fn split_for_reading(self) -> (ReadCrypto, Nonce) {
        (
            ReadCrypto {
                remote_nonce: self.nonce_pair.remote,
                precomputed_key: self.precomputed_key,
            },
            self.nonce_pair.local,
        )
    }

    pub fn unsplit_after_reading(read_crypto: ReadCrypto, local_nonce: Nonce) -> Self {
        Self {
            precomputed_key: read_crypto.precomputed_key,
            nonce_pair: NoncePair {
                local: local_nonce,
                remote: read_crypto.remote_nonce,
            },
        }
    }

    pub fn split_for_writing(self) -> (WriteCrypto, Nonce) {
        (
            WriteCrypto {
                local_nonce: self.nonce_pair.local,
                precomputed_key: self.precomputed_key,
            },
            self.nonce_pair.remote,
        )
    }

    pub fn unsplit_after_writing(write_crypto: WriteCrypto, remote_nonce: Nonce) -> Self {
        Self {
            precomputed_key: write_crypto.precomputed_key,
            nonce_pair: NoncePair {
                local: write_crypto.local_nonce,
                remote: remote_nonce,
            },
        }
    }

    pub fn split(self) -> (ReadCrypto, WriteCrypto) {
        (
            ReadCrypto {
                remote_nonce: self.nonce_pair.remote,
                precomputed_key: self.precomputed_key.clone(),
            },
            WriteCrypto {
                local_nonce: self.nonce_pair.local,
                precomputed_key: self.precomputed_key,
            },
        )
    }
}
