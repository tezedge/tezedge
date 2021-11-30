// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! This crate provides functions for manipulation with 'public key'.
//! Tezos uses this kinds: edpk(ed25519), sppk(secp256k1), p2pk(p256)

use std::{
    convert::{TryFrom, TryInto},
    str::FromStr,
};

use crypto::{
    blake2b,
    hash::{
        ChainId, ContractTz1Hash, ContractTz2Hash, ContractTz3Hash, HashTrait, PublicKeyEd25519,
        PublicKeyP256, PublicKeySecp256k1, Signature,
    },
    CryptoError, PublicKeySignatureVerifier,
};

use crate::base::ConversionError;

use super::SignatureCurve;
use tezos_encoding::{enc::BinWriter, encoding::HasEncoding, nom::NomReader};

/// Signature watermark that is prepended to the bytes for signing and verifying.
///
/// It is
/// - 0x01 + chain id for a block header signature,
/// - 0x02 + chain id for an endorsement signature,
/// - 0x03 for other operations
#[derive(Clone, Debug)]
pub enum SignatureWatermark {
    BlockHeader(ChainId),
    Endorsement(ChainId),
    GenericOperation,
    Custom(Vec<u8>),
    None,
}

/// This is a wrapper for Signature.PublicKey, which tezos uses with different curves: edpk(ed25519), sppk(secp256k1), p2pk(p256) and smart contracts
#[derive(Clone, PartialEq, Eq, Hash, HasEncoding, NomReader, BinWriter)]
pub enum SignaturePublicKey {
    Ed25519(PublicKeyEd25519),
    Secp256k1(PublicKeySecp256k1),
    P256(PublicKeyP256),
}

impl std::fmt::Debug for SignaturePublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_representation())
    }
}

impl SignaturePublicKey {
    #[inline]
    pub fn to_string_representation(&self) -> String {
        match self {
            SignaturePublicKey::Ed25519(h) => h.to_base58_check(),
            SignaturePublicKey::Secp256k1(h) => h.to_base58_check(),
            SignaturePublicKey::P256(h) => h.to_base58_check(),
        }
    }

    #[inline]
    pub fn from_b58_hash(b58_hash: &str) -> Result<SignaturePublicKey, ConversionError> {
        if b58_hash.len() > 4 {
            match &b58_hash[0..4] {
                "edpk" => Ok(SignaturePublicKey::Ed25519(b58_hash.try_into()?)),
                "sppk" => Ok(SignaturePublicKey::Secp256k1(b58_hash.try_into()?)),
                "p2pk" => Ok(SignaturePublicKey::P256(b58_hash.try_into()?)),
                _ => Err(ConversionError::InvalidCurveTag {
                    curve_tag: String::from(&b58_hash[0..4]),
                }),
            }
        } else {
            Err(ConversionError::InvalidHash {
                hash: b58_hash.to_string(),
            })
        }
    }

    #[inline]
    pub fn from_hash_and_curve(
        hash: &[u8],
        curve: SignatureCurve,
    ) -> Result<Self, ConversionError> {
        if hash.len() == 32 || hash.len() == 33 {
            let public_hash_key = match curve {
                SignatureCurve::Ed25519 => Self::Ed25519(hash.try_into()?),
                SignatureCurve::Secp256k1 => Self::Secp256k1(hash.try_into()?),
                SignatureCurve::P256 => Self::P256(hash.try_into()?),
            };
            Ok(public_hash_key)
        } else {
            Err(ConversionError::InvalidHash {
                hash: hex::encode(hash).to_string(),
            })
        }
    }

    #[inline]
    pub fn from_hex_hash_and_curve(hash: &str, curve: &str) -> Result<Self, ConversionError> {
        if hash.len() == 64 || hash.len() == 66 {
            Self::from_hash_and_curve(
                &hex::decode(hash)?,
                SignatureCurve::from_str(curve).map_err(|_| ConversionError::InvalidCurveTag {
                    curve_tag: curve.to_string(),
                })?,
            )
        } else {
            Err(ConversionError::InvalidHash {
                hash: hash.to_string(),
            })
        }
    }

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
            let tag = pk[0];
            let bytes = &pk[1..];
            match tag {
                0 => Self::from_hash_and_curve(bytes, SignatureCurve::Ed25519),
                1 => Self::from_hash_and_curve(bytes, SignatureCurve::Secp256k1),
                2 => Self::from_hash_and_curve(bytes, SignatureCurve::P256),
                _ => Err(ConversionError::InvalidPublicKey),
            }
        } else {
            Err(ConversionError::InvalidPublicKey)
        }
    }

    pub fn verify_signature<B>(
        &self,
        signature: &Signature,
        watermark: SignatureWatermark,
        bytes: B,
    ) -> Result<bool, CryptoError>
    where
        B: AsRef<[u8]>,
    {
        // TODO directly use `sodiumoxide::generichash` to avoid constructing single slice to be hashed
        let bytes_ref = bytes.as_ref();
        let bytes = match watermark {
            SignatureWatermark::BlockHeader(chain_id) => {
                let mut bytes = Vec::with_capacity(1 + ChainId::hash_size() + bytes_ref.len());
                bytes.push(0x01);
                bytes.extend_from_slice(chain_id.as_ref());
                bytes.extend_from_slice(bytes_ref);
                bytes
            }
            SignatureWatermark::Endorsement(chain_id) => {
                let mut bytes = Vec::with_capacity(1 + ChainId::hash_size() + bytes_ref.len());
                bytes.push(0x02);
                bytes.extend_from_slice(chain_id.as_ref());
                bytes.extend_from_slice(bytes_ref);
                bytes
            }
            SignatureWatermark::GenericOperation => {
                let mut bytes = Vec::with_capacity(1 + bytes_ref.len());
                bytes.push(0x03);
                bytes.extend_from_slice(bytes_ref);
                bytes
            }
            SignatureWatermark::Custom(prefix) => {
                let mut bytes = Vec::with_capacity(prefix.len() + bytes_ref.len());
                bytes.extend_from_slice(&prefix);
                bytes.extend_from_slice(bytes_ref);
                bytes
            }
            SignatureWatermark::None => bytes_ref.to_vec(),
        };
        let hash = blake2b::digest_256(&bytes)
            .map_err(|_| ())
            .map_err(|_| CryptoError::InvalidMessage)?;
        match self {
            SignaturePublicKey::Ed25519(pk) => pk.verify_signature(signature, &hash),
            SignaturePublicKey::Secp256k1(pk) => pk.verify_signature(signature, &hash),
            SignaturePublicKey::P256(pk) => pk.verify_signature(signature, &hash),
        }
    }
}

impl serde::Serialize for SignaturePublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string_representation())
    }
}

impl<'de> serde::Deserialize<'de> for SignaturePublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SignatureVisitor;

        impl<'de> serde::de::Visitor<'de> for SignatureVisitor {
            type Value = SignaturePublicKey;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("base58 encoded data")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Self::Value::from_b58_hash(v)
                    .map_err(|e| E::custom(format!("cannot convert from base58: {}", e)))
            }
        }

        deserializer.deserialize_string(SignatureVisitor)
    }
}

/// This is a wrapper for Signature.PublicKeyHash, which tezos uses with different curves: tz1(ed25519), tz2 (secp256k1), tz3(p256).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, HasEncoding, NomReader)]
pub enum SignaturePublicKeyHash {
    Ed25519(ContractTz1Hash),
    Secp256k1(ContractTz2Hash),
    P256(ContractTz3Hash),
}

impl std::fmt::Debug for SignaturePublicKeyHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string_representation())
    }
}

impl SignaturePublicKeyHash {
    #[inline]
    pub fn to_string_representation(&self) -> String {
        match self {
            SignaturePublicKeyHash::Ed25519(h) => h.to_base58_check(),
            SignaturePublicKeyHash::Secp256k1(h) => h.to_base58_check(),
            SignaturePublicKeyHash::P256(h) => h.to_base58_check(),
        }
    }

    #[inline]
    fn from_hash_and_curve(hash: &[u8], curve: SignatureCurve) -> Result<Self, ConversionError> {
        if hash.len() == 20 {
            let public_hash_key = match curve {
                SignatureCurve::Ed25519 => Self::Ed25519(hash.try_into()?),
                SignatureCurve::Secp256k1 => Self::Secp256k1(hash.try_into()?),
                SignatureCurve::P256 => Self::P256(hash.try_into()?),
            };
            Ok(public_hash_key)
        } else {
            Err(ConversionError::InvalidHash {
                hash: hex::encode(hash).to_string(),
            })
        }
    }

    #[inline]
    pub fn from_hex_hash_and_curve(hash: &str, curve: &str) -> Result<Self, ConversionError> {
        if hash.len() == 40 {
            Self::from_hash_and_curve(
                &hex::decode(hash)?,
                SignatureCurve::from_str(curve).map_err(|_| ConversionError::InvalidCurveTag {
                    curve_tag: curve.to_string(),
                })?,
            )
        } else {
            Err(ConversionError::InvalidHash {
                hash: hash.to_string(),
            })
        }
    }

    #[inline]
    pub fn from_b58_hash(b58_hash: &str) -> Result<SignaturePublicKeyHash, ConversionError> {
        if b58_hash.len() > 3 {
            match &b58_hash[0..3] {
                "tz1" => Ok(SignaturePublicKeyHash::Ed25519(
                    ContractTz1Hash::from_base58_check(b58_hash)?,
                )),
                "tz2" => Ok(SignaturePublicKeyHash::Secp256k1(
                    ContractTz2Hash::from_base58_check(b58_hash)?,
                )),
                "tz3" => Ok(SignaturePublicKeyHash::P256(
                    ContractTz3Hash::from_base58_check(b58_hash)?,
                )),
                _ => Err(ConversionError::InvalidCurveTag {
                    curve_tag: String::from(&b58_hash[0..3]),
                }),
            }
        } else {
            Err(ConversionError::InvalidHash {
                hash: b58_hash.to_string(),
            })
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
        SignaturePublicKey::from_tagged_bytes(pk)?
            .try_into()
            .map_err(|err| match err {
                crypto::hash::TryFromPKError::Digest(err) => err.into(),
                crypto::hash::TryFromPKError::Size(err) => err.into(),
            })
    }
}

impl TryFrom<SignaturePublicKey> for SignaturePublicKeyHash {
    type Error = crypto::hash::TryFromPKError;

    fn try_from(source: SignaturePublicKey) -> Result<Self, Self::Error> {
        Ok(match source {
            SignaturePublicKey::Ed25519(key) => SignaturePublicKeyHash::Ed25519(key.try_into()?),
            SignaturePublicKey::Secp256k1(key) => {
                SignaturePublicKeyHash::Secp256k1(key.try_into()?)
            }
            SignaturePublicKey::P256(key) => SignaturePublicKeyHash::P256(key.try_into()?),
        })
    }
}

impl serde::Serialize for SignaturePublicKeyHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string_representation())
    }
}

impl<'de> serde::Deserialize<'de> for SignaturePublicKeyHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct SignatureVisitor;

        impl<'de> serde::de::Visitor<'de> for SignatureVisitor {
            type Value = SignaturePublicKeyHash;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("base58 encoded data")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Self::Value::from_b58_hash(v)
                    .map_err(|e| E::custom(format!("cannot convert from base58: {}", e)))
            }
        }

        deserializer.deserialize_string(SignatureVisitor)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crypto::hash::{PublicKeyEd25519, PublicKeyP256, PublicKeySecp256k1};

    use crate::base::ConversionError;

    use super::{SignaturePublicKey, SignaturePublicKeyHash};

    //tz1gk3TDbU7cJuiBRMhwQXVvgDnjsxuWhcEA - edpkv2CiwuithtFAYEvH3QKfrJkq4JZuL4YS7i9W1vaKFfHZHLP2JP
    //tz2TSvNTh2epDMhZHrw73nV9piBX7kLZ9K9m - sppk7bn9MKAWDUFwqowcxA1zJgp12yn2kEnMQJP3WmqSZ4W8WQhLqJN
    //tz3bEQoFCZEEfZMskefZ8q8e4eiHH1pssRax - p2pk66G3vbHoscNYJdgQU72xSkrCWzoXNnFwroADcRTUtrHDvwnUNyW

    #[test]
    fn test_from_b58_hash() -> Result<(), anyhow::Error> {
        let decoded = decode_pk("edpkv2CiwuithtFAYEvH3QKfrJkq4JZuL4YS7i9W1vaKFfHZHLP2JP");
        assert!(decoded.is_some());

        let expected_valid_bytes = [
            181, 154, 48, 170, 159, 163, 206, 35, 84, 17, 234, 205, 0, 80, 66, 141, 114, 204, 77,
            76, 204, 108, 83, 76, 39, 206, 128, 206, 247, 170, 72, 113,
        ];
        let decoded = decoded.unwrap();
        assert_eq!(decoded, expected_valid_bytes);
        println!("{}", hex::encode(&expected_valid_bytes));

        let decoded = decode_pk("sppk7bn9MKAWDUFwqowcxA1zJgp12yn2kEnMQJP3WmqSZ4W8WQhLqJN");
        assert!(decoded.is_some());

        let expected_valid_bytes = vec![
            3, 69, 218, 140, 119, 94, 111, 160, 6, 57, 131, 202, 89, 148, 129, 168, 50, 167, 185,
            155, 207, 103, 33, 252, 0, 241, 225, 115, 122, 207, 237, 248, 197,
        ];
        let decoded = decoded.unwrap();
        assert_eq!(decoded, expected_valid_bytes);
        println!("{}", hex::encode(&expected_valid_bytes));

        let decoded = decode_pk("p2pk66G3vbHoscNYJdgQU72xSkrCWzoXNnFwroADcRTUtrHDvwnUNyW");
        assert!(decoded.is_some());

        let expected_valid_bytes = vec![
            2, 222, 68, 156, 232, 235, 225, 129, 250, 81, 252, 118, 231, 35, 189, 211, 21, 80, 11,
            178, 189, 66, 159, 204, 141, 166, 10, 113, 79, 176, 188, 68, 228,
        ];
        let decoded = decoded.unwrap();
        assert_eq!(decoded, expected_valid_bytes);
        println!("{}", hex::encode(&expected_valid_bytes));

        Ok(())
    }

    #[test]
    fn test_from_hex_hash_and_curve() -> Result<(), anyhow::Error> {
        println!(
            "{}",
            hex::encode(
                PublicKeyEd25519::from_base58_check(
                    "edpkv2CiwuithtFAYEvH3QKfrJkq4JZuL4YS7i9W1vaKFfHZHLP2JP"
                )
                .unwrap()
                .as_ref()
            )
        );

        let result = SignaturePublicKey::from_hex_hash_and_curve(
            &"b59a30aa9fa3ce235411eacd0050428d72cc4d4ccc6c534c27ce80cef7aa4871",
            &"ed25519",
        )
        .unwrap();
        assert_eq!(
            result.to_string_representation().as_str(),
            "edpkv2CiwuithtFAYEvH3QKfrJkq4JZuL4YS7i9W1vaKFfHZHLP2JP"
        );

        let result = SignaturePublicKey::from_hex_hash_and_curve(
            &"0345da8c775e6fa0063983ca599481a832a7b99bcf6721fc00f1e1737acfedf8c5",
            &"secp256k1",
        )
        .unwrap();
        assert_eq!(
            result.to_string_representation().as_str(),
            "sppk7bn9MKAWDUFwqowcxA1zJgp12yn2kEnMQJP3WmqSZ4W8WQhLqJN"
        );

        let result = SignaturePublicKey::from_hex_hash_and_curve(
            &"02de449ce8ebe181fa51fc76e723bdd315500bb2bd429fcc8da60a714fb0bc44e4",
            &"p256",
        )
        .unwrap();
        assert_eq!(
            result.to_string_representation().as_str(),
            "p2pk66G3vbHoscNYJdgQU72xSkrCWzoXNnFwroADcRTUtrHDvwnUNyW"
        );

        Ok(())
    }

    fn decode_pk(pk: &str) -> Option<Vec<u8>> {
        let decoded = SignaturePublicKey::from_b58_hash(pk).unwrap();
        Some(match decoded {
            SignaturePublicKey::Ed25519(PublicKeyEd25519(hash))
            | SignaturePublicKey::P256(PublicKeyP256(hash))
            | SignaturePublicKey::Secp256k1(PublicKeySecp256k1(hash)) => hash,
        })
    }

    #[test]
    fn test_ed25519_from_bytes() -> Result<(), anyhow::Error> {
        let valid_pk = vec![
            0, 3, 65, 14, 206, 174, 244, 127, 36, 48, 150, 156, 243, 27, 213, 139, 41, 30, 231,
            173, 127, 97, 192, 177, 142, 31, 107, 197, 219, 246, 111, 155, 121,
        ];
        let short_pk = vec![
            0, 3, 65, 14, 206, 174, 244, 127, 36, 48, 150, 156, 243, 27, 213, 139, 41, 30, 231,
            173, 127, 97, 192, 177, 142, 31, 107, 197, 219,
        ];
        let wrong_tag_pk = vec![
            4, 3, 65, 14, 206, 174, 244, 127, 36, 48, 150, 156, 243, 27, 213, 139, 41, 30, 231,
            173, 127, 97, 192, 177, 142, 31, 107, 197, 219, 246, 111, 155, 121,
        ];

        let decoded = SignaturePublicKeyHash::from_tagged_bytes(valid_pk).unwrap();
        let decoded = match decoded {
            SignaturePublicKeyHash::Ed25519(hash) => Some(hash),
            _ => None,
        };
        assert!(decoded.is_some());

        let decoded = decoded.map(|h| h.to_base58_check()).unwrap();
        assert_eq!("tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17", decoded);

        let result = SignaturePublicKeyHash::from_tagged_bytes(short_pk);
        assert_eq!(result, Err(ConversionError::InvalidPublicKey));

        let result = SignaturePublicKeyHash::from_tagged_bytes(wrong_tag_pk);
        assert_eq!(result, Err(ConversionError::InvalidPublicKey));

        Ok(())
    }

    #[test]
    fn test_ed25519_from_b58_hash() -> Result<(), anyhow::Error> {
        let decoded =
            SignaturePublicKeyHash::from_b58_hash("tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17")?;
        let decoded = match decoded {
            SignaturePublicKeyHash::Ed25519(hash) => Some(hash),
            _ => None,
        };
        assert!(decoded.is_some());

        let expected_valid_pk = hex::decode("2cca28ab019ae2d8c26f4ce4924cad67a2dc6618")?;
        let decoded = decoded.unwrap();
        assert_eq!(decoded.as_ref(), &expected_valid_pk);
        Ok(())
    }

    #[test]
    fn test_try_from_pk() -> Result<(), anyhow::Error> {
        let pk_ed25519 = SignaturePublicKey::from_b58_hash(
            "edpkv2CiwuithtFAYEvH3QKfrJkq4JZuL4YS7i9W1vaKFfHZHLP2JP",
        )?;
        let pkh_ed25519 =
            SignaturePublicKeyHash::from_b58_hash("tz1gk3TDbU7cJuiBRMhwQXVvgDnjsxuWhcEA")?;
        assert_eq!(SignaturePublicKeyHash::try_from(pk_ed25519)?, pkh_ed25519);

        let pk_secp256k1 = SignaturePublicKey::from_b58_hash(
            "sppk7bn9MKAWDUFwqowcxA1zJgp12yn2kEnMQJP3WmqSZ4W8WQhLqJN",
        )?;
        let pkh_secp256k1 =
            SignaturePublicKeyHash::from_b58_hash("tz2TSvNTh2epDMhZHrw73nV9piBX7kLZ9K9m")?;
        assert_eq!(
            SignaturePublicKeyHash::try_from(pk_secp256k1)?,
            pkh_secp256k1
        );

        let pk_p256 = SignaturePublicKey::from_b58_hash(
            "p2pk66G3vbHoscNYJdgQU72xSkrCWzoXNnFwroADcRTUtrHDvwnUNyW",
        )?;
        let pkh_p256 =
            SignaturePublicKeyHash::from_b58_hash("tz3bEQoFCZEEfZMskefZ8q8e4eiHH1pssRax")?;
        assert_eq!(SignaturePublicKeyHash::try_from(pk_p256)?, pkh_p256);

        Ok(())
    }

    #[test]
    fn test_hash_from_hex_hash_and_curve() -> Result<(), anyhow::Error> {
        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(
            &"2cca28ab019ae2d8c26f4ce4924cad67a2dc6618",
            &"ed25519",
        )
        .unwrap();
        assert_eq!(
            result.to_string_representation().as_str(),
            "tz1PirboZKFVqkfE45hVLpkpXaZtLk3mqC17"
        );

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(
            &"20262e6195b91181f1713c4237c8195096b8adc9",
            &"secp256k1",
        )
        .unwrap();
        assert_eq!(
            result.to_string_representation().as_str(),
            "tz2BFE2MEHhphgcR7demCGQP2k1zG1iMj1oj"
        );

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(
            &"6fde46af0356a0476dae4e4600172dc9309b3aa4",
            &"p256",
        )
        .unwrap();
        assert_eq!(
            result.to_string_representation().as_str(),
            "tz3WXYtyDUNL91qfiCJtVUX746QpNv5i5ve5"
        );

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(
            &"2cca28ab019ae2d8c26f4ce4924cad67a2dc6618",
            &"invalidcurvetag",
        );
        assert_eq!(
            result.unwrap_err(),
            ConversionError::InvalidCurveTag {
                curve_tag: "invalidcurvetag".to_string()
            }
        );

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(
            &"2cca28a6f4ce4924cad67a2dc6618",
            &"ed25519",
        );
        assert_eq!(
            result.unwrap_err(),
            ConversionError::InvalidHash {
                hash: "2cca28a6f4ce4924cad67a2dc6618".to_string()
            }
        );

        let result = SignaturePublicKeyHash::from_hex_hash_and_curve(
            &"56a0476dae4e4600172dc9309b3aa4",
            &"p256",
        );
        assert_eq!(
            result.unwrap_err(),
            ConversionError::InvalidHash {
                hash: "56a0476dae4e4600172dc9309b3aa4".to_string()
            }
        );

        Ok(())
    }
}
