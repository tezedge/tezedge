// Copyright (c) SimpleStaking, Viable Systems, TriliTech and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This module wrapps [`sodiumoxide::crypto::box_::`] stuff,
//! which is used for encrypt/decrypt messages between peers.
//!
//! Terminology:
//!
//! PublicKey - [`CRYPTO_KEY_SIZE`]-bytes
//! SecretKey - [`CRYPTO_KEY_SIZE`]-bytes
//! PrecomputedKey - [`CRYPTO_KEY_SIZE`]-bytes created from PublicKey and SecretKey
//!
//! CryptoboxPublicKeyHash - generated as a hash of [`PublicKey`], for example used as a peer_id

use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt::{self, Debug};

use hex::{FromHex, FromHexError};
use sodiumoxide::crypto::box_;

use crate::{blake2b::Blake2bError, hash::FromBytesError, CryptoError};

use super::hash::CryptoboxPublicKeyHash;

use thiserror::Error;

pub const BOX_ZERO_BYTES: usize = 32;
pub const CRYPTO_KEY_SIZE: usize = 32;

pub trait CryptoKey: Sized {
    fn from_bytes<B: AsRef<[u8]>>(buf: B) -> Result<Self, CryptoError>;
}

#[derive(Debug, Error, PartialEq)]
pub enum PublicKeyError {
    #[error("Error constructing hash: {0}")]
    HashError(#[from] FromBytesError),
    #[error("Blake2b digest error: {0}")]
    Blake2bError(#[from] Blake2bError),
}

fn ensure_crypto_key_bytes<B: AsRef<[u8]>>(buf: B) -> Result<[u8; CRYPTO_KEY_SIZE], CryptoError> {
    let buf = buf.as_ref();

    // check size
    if buf.len() != CRYPTO_KEY_SIZE {
        return Err(CryptoError::InvalidKeySize {
            expected: CRYPTO_KEY_SIZE,
            actual: buf.len(),
        });
    };

    // convert to correct key size
    let mut arr = [0u8; CRYPTO_KEY_SIZE];
    arr.copy_from_slice(buf);
    Ok(arr)
}

/// Convenience wrapper around [`sodiumoxide::crypto::box_::PublicKey`]
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
pub struct PublicKey(box_::PublicKey);

impl PublicKey {
    /// Generates public key hash for public key
    pub fn public_key_hash(&self) -> Result<CryptoboxPublicKeyHash, PublicKeyError> {
        CryptoboxPublicKeyHash::try_from(crate::blake2b::digest_128(self.0.as_ref())?)
            .map_err(PublicKeyError::from)
    }
}

impl CryptoKey for PublicKey {
    fn from_bytes<B: AsRef<[u8]>>(buf: B) -> Result<Self, CryptoError> {
        ensure_crypto_key_bytes(buf).map(|key_bytes| PublicKey(box_::PublicKey(key_bytes)))
    }
}

impl AsRef<box_::PublicKey> for PublicKey {
    fn as_ref(&self) -> &box_::PublicKey {
        &self.0
    }
}

impl FromHex for PublicKey {
    type Error = CryptoError;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        Self::from_bytes(hex::decode(hex)?)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
/// Convenience wrapper around [`sodiumoxide::crypto::box_::SecretKey`]
pub struct SecretKey(box_::SecretKey);

impl CryptoKey for SecretKey {
    fn from_bytes<B: AsRef<[u8]>>(buf: B) -> Result<Self, CryptoError> {
        ensure_crypto_key_bytes(buf).map(|key_bytes| SecretKey(box_::SecretKey(key_bytes)))
    }
}

impl AsRef<box_::SecretKey> for SecretKey {
    fn as_ref(&self) -> &box_::SecretKey {
        &self.0
    }
}

impl FromHex for SecretKey {
    type Error = CryptoError;

    fn from_hex<T: AsRef<[u8]>>(hex: T) -> Result<Self, Self::Error> {
        Self::from_bytes(hex::decode(hex)?)
    }
}

/// Generates random keypair: [`PublicKey, SecretKey`] + [`CryptoboxPublicKeyHash`]
///
/// Note: Strange why it is called pair, bud returns triplet :)
pub fn random_keypair() -> Result<(SecretKey, PublicKey, CryptoboxPublicKeyHash), PublicKeyError> {
    // generate
    let (pk, sk) = box_::gen_keypair();

    // wrap it
    let sk = SecretKey(sk);
    let pk = PublicKey(pk);

    // generate public key hash
    let pkh = pk.public_key_hash()?;

    // return
    Ok((sk, pk, pkh))
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Clone)]
/// Convenience wrapper around [`sodiumoxide::crypto::box_::PrecomputedKey`]
pub struct PrecomputedKey(box_::PrecomputedKey);

#[cfg(feature = "fuzzing")]
impl fuzzcheck::DefaultMutator for PrecomputedKey {
    type Mutator = fuzzcheck::mutators::unit::UnitMutator<PrecomputedKey>;
    #[no_coverage]
    fn default_mutator() -> Self::Mutator {
        let pk = PrecomputedKey::from_bytes([0; box_::PRECOMPUTEDKEYBYTES]);
        fuzzcheck::mutators::unit::UnitMutator::new(pk)
    }
}

impl PrecomputedKey {
    /// Create `PrecomputedKey` from public key and secret key
    ///
    /// # Arguments
    /// * `pk_as_hex_string` - Hex string representing public key
    /// * `sk_as_hex_string` - Hex string representing secret key
    pub fn precompute(pk: &PublicKey, sk: &SecretKey) -> Self {
        Self(box_::precompute(pk.as_ref(), sk.as_ref()))
    }

    pub fn from_bytes(bytes: [u8; box_::PRECOMPUTEDKEYBYTES]) -> Self {
        Self(box_::PrecomputedKey(bytes))
    }
}

impl Debug for PrecomputedKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PrecomputedKey(****)")
    }
}

impl From<FromHexError> for CryptoError {
    fn from(e: FromHexError) -> Self {
        CryptoError::InvalidKey {
            reason: format!("{}", e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_precomputed_key() -> Result<(), anyhow::Error> {
        let pk = PublicKey::from_hex(
            "96678b88756dd6cfd6c129980247b70a6e44da77823c3672a2ec0eae870d8646",
        )?;
        let sk = SecretKey::from_hex(
            "a18dc11cb480ebd31081e1541df8bd70c57da0fa419b5036242f8619d605e75a",
        )?;
        let pck = PrecomputedKey::precompute(&pk, &sk);

        let precomputed = hex::encode(&pck.0);
        let expected_precomputed =
            "5228751a6f5a6494e38e1042f578e3a64ae3462b7899356f49e50be846c9609c";
        assert_eq!(expected_precomputed, precomputed);

        Ok(())
    }
}
