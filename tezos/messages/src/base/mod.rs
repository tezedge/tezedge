// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::str::FromStr;

use crypto::blake2b::Blake2bError;
use hex::FromHexError;
use thiserror::Error;

use crypto::base58::FromBase58CheckError;
use crypto::hash::{FromBytesError, TryFromPKError};

pub mod fitness_comparator;
pub mod rpc_support;
pub mod signature_public_key;

#[derive(Debug, strum_macros::EnumString)]
#[strum(serialize_all = "lowercase")]
pub enum SignatureCurve {
    Ed25519,
    Secp256k1,
    P256,
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Error, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub enum ConversionError {
    #[error("Conversion from invalid public key")]
    InvalidPublicKey,

    #[error("Invalid hash: {hash}")]
    InvalidHash { hash: String },

    #[error("Invalid curve tag: {curve_tag}")]
    InvalidCurveTag { curve_tag: String },

    #[error("Blake2b digest error")]
    Blake2bError,
}

impl From<hex::FromHexError> for ConversionError {
    fn from(error: FromHexError) -> Self {
        ConversionError::InvalidHash {
            hash: error.to_string(),
        }
    }
}

impl From<FromBase58CheckError> for ConversionError {
    fn from(error: FromBase58CheckError) -> Self {
        ConversionError::InvalidHash {
            hash: error.to_string(),
        }
    }
}

impl From<FromBytesError> for ConversionError {
    fn from(error: FromBytesError) -> Self {
        ConversionError::InvalidHash {
            hash: error.to_string(),
        }
    }
}

impl From<Blake2bError> for ConversionError {
    fn from(_: Blake2bError) -> Self {
        ConversionError::Blake2bError
    }
}

impl From<<SignatureCurve as FromStr>::Err> for ConversionError {
    fn from(error: <SignatureCurve as FromStr>::Err) -> Self {
        Self::InvalidCurveTag {
            curve_tag: error.to_string(),
        }
    }
}

impl From<TryFromPKError> for ConversionError {
    fn from(error: TryFromPKError) -> Self {
        match error {
            TryFromPKError::Digest(_) => Self::Blake2bError,
            TryFromPKError::Size(e) => Self::InvalidHash {
                hash: e.to_string(),
            },
        }
    }
}
