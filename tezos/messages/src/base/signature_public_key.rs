// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This crate provides functions for manipulation with 'public key'.
//! Tezos uses this kinds: edpk(ed25519), sppk(secp256k1), p2pk(p256)

use serde::{Deserialize, Serialize};

use crypto::hash::{PublicKeyEd25519, PublicKeySecp256k1, PublicKeyP256, HashType};

use crate::base::ConversionError;

/// This is a wrapper for Signature.PublicKey, which tezos uses with different curves: edpk(ed25519), sppk(secp256k1), p2pk(p256) and smart contracts
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum SignaturePublicKey {
    Ed25519(PublicKeyEd25519),
    Secp256k1(PublicKeySecp256k1),
    P256(PublicKeyP256),
}

impl SignaturePublicKey {
    #[inline]
    pub fn to_string(&self) -> String {
        match self {
            SignaturePublicKey::Ed25519(h) => HashType::PublicKeyEd25519.bytes_to_string(h),
            SignaturePublicKey::Secp256k1(h) => HashType::PublicKeySecp256k1.bytes_to_string(h),
            SignaturePublicKey::P256(h) => HashType::PublicKeyP256.bytes_to_string(h),
        }
    }

    #[inline]
    pub fn from_b58_hash(b58_hash: &str) -> Result<SignaturePublicKey, ConversionError> {
        if b58_hash.len() > 4 {
            match &b58_hash[0..4] {
                "edpk" => {
                    Ok(SignaturePublicKey::Ed25519(HashType::PublicKeyEd25519.string_to_bytes(b58_hash)?))
                }
                "sppk" => {
                    Ok(SignaturePublicKey::Secp256k1(HashType::PublicKeySecp256k1.string_to_bytes(b58_hash)?))
                }
                "p2pk" => {
                    Ok(SignaturePublicKey::P256(HashType::PublicKeyP256.string_to_bytes(b58_hash)?))
                }
                _ => return Err(ConversionError::InvalidCurveTag { curve_tag: String::from(&b58_hash[0..4]) })
            }
        } else {
            return Err(ConversionError::InvalidHash { hash: b58_hash.to_string() });
        }
    }

    #[inline]
    pub fn from_hex_hash_and_curve(hash: &str, curve: &str) -> Result<SignaturePublicKey, ConversionError> {
        if hash.len() == 66 || hash.len() == 64 {
            let public_hash_key = match curve {
                "ed25519" => {
                    let key = HashType::PublicKeyEd25519.bytes_to_string(&hex::decode(&hash)?);
                    SignaturePublicKey::Ed25519(HashType::PublicKeyEd25519.string_to_bytes(&key)?)
                }
                "secp256k1" => {
                    let key = HashType::PublicKeySecp256k1.bytes_to_string(&hex::decode(&hash)?);
                    SignaturePublicKey::Secp256k1(HashType::PublicKeySecp256k1.string_to_bytes(&key)?)
                }
                "p256" => {
                    let key = HashType::PublicKeyP256.bytes_to_string(&hex::decode(&hash)?);
                    SignaturePublicKey::P256(HashType::PublicKeyP256.string_to_bytes(&key)?)
                }
                _ => return Err(ConversionError::InvalidCurveTag { curve_tag: curve.to_string() })
            };
            Ok(public_hash_key)
        } else {
            return Err(ConversionError::InvalidHash { hash: hash.to_string() });
        }
    }
    // 

    /// convert public key byte string to contract id, e.g. data from context can be in this format
    ///
    /// 1 byte tag and - 32 bytes for ed25519 (tz1)
    ///                - 33 bytes for secp256k1 (tz2) and p256 (tz3)
    ///
    /// # Arguments
    ///
    /// * `pk` - public key in byte string format
    #[inline]
    pub fn from_tagged_bytes(pk: Vec<u8>) -> Result<SignaturePublicKey, ConversionError> {
        if pk.len() == 33 || pk.len() == 34 {
            let tag = pk[1];

            match tag {
                0 => {
                    Self::from_hex_hash_and_curve(&hex::encode(&pk[2..]), "ed25519")
                }
                1 => {
                    Self::from_hex_hash_and_curve(&hex::encode(&pk[2..]), "secp256k1")
                }
                2 => {
                    Self::from_hex_hash_and_curve(&hex::encode(&pk[2..]), "p256")
                }
                _ => return Err(ConversionError::InvalidPublicKey)
            }
        } else {
            return Err(ConversionError::InvalidPublicKey);
        }
    }

    #[inline]
    pub fn from_tagged_hex_string(pk: &str) -> Result<SignaturePublicKey, ConversionError> {
        if pk.len() == 66 || pk.len() == 64 {
            // first byte is a tag for contract implicitness
            match &pk[0..2] {
                "00" => {
                    let tagless = &pk[4..];
                    match &pk[2..4] {
                        "00" => {
                            Self::from_hex_hash_and_curve(&tagless, "ed25519")
                        }
                        "01" => {
                            Self::from_hex_hash_and_curve(&tagless, "secp256k1")
                        }
                        "02" =>{
                            Self::from_hex_hash_and_curve(&tagless, "p256")
                        }
                        _ => return Err(ConversionError::InvalidHash{ hash: pk.to_string() })
                    }
                }
                _ => return Err(ConversionError::InvalidHash{ hash: pk.to_string() })
            }
        } else {
            return Err(ConversionError::InvalidHash{ hash: pk.to_string() });
        }
    }
}

#[cfg(test)]
mod tests {
//TODO
    
}