// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;
use hex::FromHexError;

use crypto::base58::FromBase58CheckError;
use crypto::hash::FromBytesError;

pub mod fitness_comparator;
pub mod rpc_support;
pub mod signature_public_key;
pub mod signature_public_key_hash;

#[derive(Debug, Fail, PartialEq)]
pub enum ConversionError {
    #[fail(display = "Conversion from invalid public key")]
    InvalidPublicKey,

    #[fail(display = "Invalid hash: {}", hash)]
    InvalidHash { hash: String },

    #[fail(display = "Invalid curve tag: {}", curve_tag)]
    InvalidCurveTag { curve_tag: String },
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
