// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-CopyrightText: 2023 TriliTech <contact@trili.tech>
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

use thiserror::Error;

#[macro_use]
pub mod blake2b;
pub mod base58;
pub mod bls;
#[macro_use]
pub mod hash;

#[derive(Debug, Error, PartialEq)]
pub enum CryptoError {
    #[error("Invalid crypto key, reason: {reason}")]
    InvalidKey { reason: String },
    #[error("Invalid crypto key size - expected: {expected}, actual: {actual}")]
    InvalidKeySize { expected: usize, actual: usize },
    #[error("Invalid nonce size - expected: {expected}, actual: {actual}")]
    InvalidNonceSize { expected: usize, actual: usize },
    #[error("Failed to decrypt")]
    FailedToDecrypt,
    #[error("Failed to construct public key")]
    InvalidPublicKey,
    #[error("Failed to construct signature")]
    InvalidSignature,
    #[error("Failed to construct message")]
    InvalidMessage,
    #[error("Unsupported algorithm `{0}`")]
    Unsupported(&'static str),
    #[error("Algorithm error: `{0}`")]
    AlgorithmError(String),
}

/// Public key that support hashing.
pub trait PublicKeyWithHash {
    type Hash;
    type Error;

    fn pk_hash(&self) -> Result<Self::Hash, Self::Error>;
}

/// Public key that supports signature verification
pub trait PublicKeySignatureVerifier {
    type Signature;
    type Error;

    fn verify_signature(
        &self,
        signature: &Self::Signature,
        msg: &[u8],
    ) -> Result<bool, Self::Error>;
}
