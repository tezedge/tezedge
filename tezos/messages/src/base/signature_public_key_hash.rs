// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This crate provides functions for manipulation with 'public key hash'.
//! Tezos uses this kinds: tz1(ed25519), tz2 (secp256k1), tz3(p256)

use failure::Fail;
use hex::FromHexError;
use serde::{Deserialize, Serialize};

use crypto::base58::FromBase58CheckError;
use crypto::blake2b;
use crypto::hash::{ContractTz1Hash, ContractTz2Hash, ContractTz3Hash, HashType};

#[derive(Debug, Fail, PartialEq)]
pub enum ConversionError {
    #[fail(display = "Conversion from invalid public key")]
    InvalidPublicKey,

    #[fail(display = "Invalid hash: {}", hash)]
    InvalidHash {
        hash: String
    },

    #[fail(display = "Invalid curve tag: {}", curve_tag)]
    InvalidCurveTag {
        curve_tag: String
    },
}

impl From<hex::FromHexError> for ConversionError {
    fn from(error: FromHexError) -> Self {
        ConversionError::InvalidHash { hash: error.to_string() }
    }
}

impl From<FromBase58CheckError> for ConversionError {
    fn from(error: FromBase58CheckError) -> Self {
        ConversionError::InvalidHash { hash: error.to_string() }
    }
}

/// This is a wrapper for Signature.PublicKeyHash, which tezos uses with different curves: tz1(ed25519), tz2 (secp256k1), tz3(p256).
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum SignaturePublicKeyHash {
    Ed25519(ContractTz1Hash),
    Secp256k1(ContractTz2Hash),
    P256(ContractTz3Hash),
}

impl SignaturePublicKeyHash {
    #[inline]
    pub fn to_string(&self) -> String {
        match self {
            SignaturePublicKeyHash::Ed25519(h) => HashType::ContractTz1Hash.bytes_to_string(h),
            SignaturePublicKeyHash::Secp256k1(h) => HashType::ContractTz2Hash.bytes_to_string(h),
            SignaturePublicKeyHash::P256(h) => HashType::ContractTz3Hash.bytes_to_string(h),
        }
    }

    #[inline]
    pub fn from_hex_hash_and_curve(hash: &str, curve: &str) -> Result<SignaturePublicKeyHash, ConversionError> {
        if hash.len() == 40 {
            let public_hash_key = match curve {
                "ed25519" => {
                    let key = HashType::ContractTz1Hash.bytes_to_string(&hex::decode(&hash)?);
                    SignaturePublicKeyHash::Ed25519(HashType::ContractTz1Hash.string_to_bytes(&key)?)
                }
                "secp256k1" => {
                    let key = HashType::ContractTz2Hash.bytes_to_string(&hex::decode(&hash)?);
                    SignaturePublicKeyHash::Secp256k1(HashType::ContractTz2Hash.string_to_bytes(&key)?)
                }
                "p256" => {
                    let key = HashType::ContractTz3Hash.bytes_to_string(&hex::decode(&hash)?);
                    SignaturePublicKeyHash::P256(HashType::ContractTz3Hash.string_to_bytes(&key)?)
                }
                _ => return Err(ConversionError::InvalidCurveTag { curve_tag: curve.to_string() })
            };
            Ok(public_hash_key)
        } else {
            return Err(ConversionError::InvalidHash { hash: hash.to_string() });
        }
    }

    #[inline]
    pub fn from_b58_hash(b58_hash: &str) -> Result<SignaturePublicKeyHash, ConversionError> {
        if b58_hash.len() > 3 {
            match &b58_hash[0..3] {
                "tz1" => {
                    Ok(SignaturePublicKeyHash::Ed25519(HashType::ContractTz1Hash.string_to_bytes(b58_hash)?))
                }
                "tz2" => {
                    Ok(SignaturePublicKeyHash::Secp256k1(HashType::ContractTz2Hash.string_to_bytes(b58_hash)?))
                }
                "tz3" => {
                    Ok(SignaturePublicKeyHash::P256(HashType::ContractTz3Hash.string_to_bytes(b58_hash)?))
                }
                _ => return Err(ConversionError::InvalidCurveTag { curve_tag: String::from(&b58_hash[0..3]) })
            }
        } else {
            return Err(ConversionError::InvalidHash { hash: b58_hash.to_string() });
        }
    }

    /// convert public key byte string to contract id, e.g. data from context can be in this format
    ///
    /// 1 byte tag and - 32 bytes for ed25519 (tz1)
    ///                 - 33 bytes for secp256k1 (tz2) and p256 (tz3)
    ///
    /// # Arguments
    ///
    /// * `pk` - public key in byte string format
    #[inline]
    pub fn from_tagged_bytes(pk: Vec<u8>) -> Result<SignaturePublicKeyHash, ConversionError> {
        if pk.len() == 33 || pk.len() == 34 {
            let tag = pk[0];
            let hash = blake2b::digest_160(&pk[1..]);

            match tag {
                0 => {
                    Self::from_hex_hash_and_curve(&hex::encode(hash), "ed25519")
                }
                1 => {
                    Self::from_hex_hash_and_curve(&hex::encode(hash), "secp256k1")
                }
                2 => {
                    Self::from_hex_hash_and_curve(&hex::encode(hash), "p256")
                }
                _ => return Err(ConversionError::InvalidPublicKey)
            }
        } else {
            return Err(ConversionError::InvalidPublicKey);
        }
    }
}

#[cfg(test)]
mod tests {
    use crypto::hash::HashType;

    use crate::base::signature_public_key_hash::{ConversionError, SignaturePublicKeyHash};

    #[test]
    fn test_ed25519_from_bytes() -> Result<(), failure::Error> {
        let valid_pk = vec![0, 3, 65, 14, 206, 174, 244, 127, 36, 48, 150, 156, 243, 27, 213, 139, 41, 30, 231, 173, 127, 97, 192, 177, 142, 31, 107, 197, 219, 246, 111, 155, 121];
        let short_pk = vec![0, 3, 65, 14, 206, 174, 244, 127, 36, 48, 150, 156, 243, 27, 213, 139, 41, 30, 231, 173, 127, 97, 192, 177, 142, 31, 107, 197, 219];
        let wrong_tag_pk = vec![4, 3, 65, 14, 206, 174, 244, 127, 36, 48, 150, 156, 243, 27, 213, 139, 41, 30, 231, 173, 127, 97, 192, 177, 142, 31, 107, 197, 219, 246, 111, 155, 121];

        let decoded = SignaturePublicKeyHash::from_tagged_bytes(valid_pk.clone())?;
        let decoded = match decoded {
            SignaturePublicKeyHash::Ed25519(hash) => Some(hash),
            _ => None
        };
        assert!(decoded.is_some());

        let decoded = decoded.map(|h| HashType::ContractTz1Hash.bytes_to_string(&h)).unwrap();
        assert_eq!("tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17", decoded);

        let result = SignaturePublicKeyHash::from_tagged_bytes(short_pk);
        assert_eq!(result, Err(ConversionError::InvalidPublicKey));

        let result = SignaturePublicKeyHash::from_tagged_bytes(wrong_tag_pk);
        assert_eq!(result, Err(ConversionError::InvalidPublicKey));

        Ok(())
    }

    #[test]
    fn test_ed25519_from_b58_hash() -> Result<(), failure::Error> {
        let decoded = SignaturePublicKeyHash::from_b58_hash("tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17")?;
        let decoded = match decoded {
            SignaturePublicKeyHash::Ed25519(hash) => Some(hash),
            _ => None
        };
        assert!(decoded.is_some());

        let expected_valid_pk = hex::decode("2cca28ab019ae2d8c26f4ce4924cad67a2dc6618")?;
        let decoded = decoded.unwrap();
        assert_eq!(decoded, expected_valid_pk);
        Ok(())
    }

    #[test]
    fn test_from_hex_hash_and_curve() -> Result<(), failure::Error> {
        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(&"2cca28ab019ae2d8c26f4ce4924cad67a2dc6618", &"ed25519")?;
        assert_eq!(result.to_string().as_str(), "tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17");

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(&"20262e6195b91181f1713c4237c8195096b8adc9", &"secp256k1")?;
        assert_eq!(result.to_string().as_str(), "tz2BFE2MEHhphgcR7demCGQP2k1zG1iMj1oj");

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(&"6fde46af0356a0476dae4e4600172dc9309b3aa4", &"p256")?;
        assert_eq!(result.to_string().as_str(), "tz3WXYtyDUNL91qfiCJtVUX746QpNv5i5ve5");

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(&"2cca28ab019ae2d8c26f4ce4924cad67a2dc6618", &"invalidcurvetag");
        assert_eq!(result.unwrap_err(), ConversionError::InvalidCurveTag { curve_tag: "invalidcurvetag".to_string() });

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(&"2cca28a6f4ce4924cad67a2dc6618", &"ed25519");
        assert_eq!(result.unwrap_err(), ConversionError::InvalidHash { hash: "2cca28a6f4ce4924cad67a2dc6618".to_string() });

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(&"56a0476dae4e4600172dc9309b3aa4", &"p256");
        assert_eq!(result.unwrap_err(), ConversionError::InvalidHash { hash: "56a0476dae4e4600172dc9309b3aa4".to_string() });

        Ok(())
    }
}