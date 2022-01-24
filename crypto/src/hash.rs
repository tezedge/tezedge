// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};

use crate::{
    base58::{FromBase58Check, FromBase58CheckError, ToBase58Check},
    blake2b::{self, Blake2bError},
    crypto_box::CRYPTO_KEY_SIZE,
    CryptoError, PublicKeySignatureVerifier,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

mod prefix_bytes {
    pub const CHAIN_ID: [u8; 3] = [87, 82, 0];
    pub const BLOCK_HASH: [u8; 2] = [1, 52];
    pub const BLOCK_METADATA_HASH: [u8; 2] = [234, 249];
    pub const CONTEXT_HASH: [u8; 2] = [79, 199];
    pub const OPERATION_HASH: [u8; 2] = [5, 116];
    pub const OPERATION_LIST_LIST_HASH: [u8; 3] = [29, 159, 109];
    pub const OPERATION_METADATA_HASH: [u8; 2] = [5, 183];
    pub const OPERATION_METADATA_LIST_LIST_HASH: [u8; 3] = [29, 159, 182];
    pub const PROTOCOL_HASH: [u8; 2] = [2, 170];
    pub const CRYPTOBOX_PUBLIC_KEY_HASH: [u8; 2] = [153, 103];
    pub const CONTRACT_KT1_HASH: [u8; 3] = [2, 90, 121];
    pub const CONTRACT_TZ1_HASH: [u8; 3] = [6, 161, 159];
    pub const CONTRACT_TZ2_HASH: [u8; 3] = [6, 161, 161];
    pub const CONTRACT_TZ3_HASH: [u8; 3] = [6, 161, 164];
    pub const PUBLIC_KEY_ED25519: [u8; 4] = [13, 15, 37, 217];
    pub const PUBLIC_KEY_SECP256K1: [u8; 4] = [3, 254, 226, 86];
    pub const PUBLIC_KEY_P256: [u8; 4] = [3, 178, 139, 127];
    pub const ED22519_SIGNATURE_HASH: [u8; 5] = [9, 245, 205, 134, 18];
    pub const GENERIC_SIGNATURE_HASH: [u8; 3] = [4, 130, 43];
}

pub type Hash = Vec<u8>;

pub trait HashTrait: Into<Hash> + AsRef<Hash> {
    /// Returns this hash type.
    fn hash_type() -> HashType;

    /// Returns the size of this hash.
    fn hash_size() -> usize {
        Self::hash_type().size()
    }

    /// Tries to create this hash from the `bytes`.
    fn try_from_bytes(bytes: &[u8]) -> Result<Self, FromBytesError>;

    fn from_b58check(data: &str) -> Result<Self, FromBase58CheckError>;

    fn to_b58check(&self) -> String;
}

/// Error creating hash from bytes
#[derive(Debug, Error, PartialEq, Clone, serde::Serialize, serde::Deserialize)]
pub enum FromBytesError {
    /// Invalid data size
    #[error("invalid hash size")]
    InvalidSize,
}

macro_rules! define_hash {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub Hash);

        impl $name {
            fn from_bytes(data: &[u8]) -> Result<Self, FromBytesError> {
                if data.len() == HashType::$name.size() {
                    Ok($name(data.into()))
                } else {
                    Err(FromBytesError::InvalidSize)
                }
            }

            fn from_vec(hash: Vec<u8>) -> Result<Self, FromBytesError> {
                if hash.len() == HashType::$name.size() {
                    Ok($name(hash))
                } else {
                    Err(FromBytesError::InvalidSize)
                }
            }

            pub fn from_base58_check(data: &str) -> Result<Self, FromBase58CheckError> {
                Self::from_b58check(data)
            }

            pub fn to_base58_check(&self) -> String {
                self.to_b58check()
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                // TODO - TE-373: with b58 this could be done without the need
                // to perform a heap allocation.
                write!(f, "{}", self.to_base58_check())
            }
        }

        impl ::std::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                // TODO - TE-373: with b58 this could be done without the need
                // to perform a heap allocation.
                f.debug_tuple(stringify!($name))
                    .field(&self.to_base58_check())
                    .finish()
            }
        }

        impl std::str::FromStr for $name {
            type Err = FromBase58CheckError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_base58_check(s)
            }
        }

        impl HashTrait for $name {
            fn hash_type() -> HashType {
                HashType::$name
            }

            fn try_from_bytes(bytes: &[u8]) -> Result<Self, FromBytesError> {
                $name::try_from(bytes)
            }

            fn from_b58check(data: &str) -> Result<Self, FromBase58CheckError> {
                HashType::$name.b58check_to_hash(data).map(Self)
            }

            fn to_b58check(&self) -> String {
                // TODO: Fixing TE-373 will allow to get rid of this `unreachable`
                HashType::$name
                    .hash_to_b58check(&self.0)
                    .unwrap_or_else(|_| {
                        unreachable!("Typed hash should always be representable in base58")
                    })
            }
        }

        impl std::convert::AsRef<Hash> for $name {
            fn as_ref(&self) -> &Hash {
                &self.0
            }
        }

        impl std::convert::From<$name> for Hash {
            fn from(typed_hash: $name) -> Self {
                typed_hash.0
            }
        }

        impl std::convert::TryFrom<&[u8]> for $name {
            type Error = FromBytesError;
            fn try_from(h: &[u8]) -> Result<Self, Self::Error> {
                Self::from_bytes(h)
            }
        }

        impl std::convert::TryFrom<Hash> for $name {
            type Error = FromBytesError;
            fn try_from(h: Hash) -> Result<Self, Self::Error> {
                Self::from_vec(h)
            }
        }

        impl std::convert::TryFrom<&str> for $name {
            type Error = FromBase58CheckError;
            fn try_from(encoded: &str) -> Result<Self, Self::Error> {
                Self::from_base58_check(encoded)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                if serializer.is_human_readable() {
                    serializer.serialize_str(&self.to_base58_check())
                } else {
                    serializer.serialize_newtype_struct(stringify!($name), &self.0)
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::de::Deserializer<'de>,
            {
                struct HashVisitor;

                impl<'de> serde::de::Visitor<'de> for HashVisitor {
                    type Value = $name;

                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter
                            .write_str("eigher sequence of bytes or base58 encoded data expected")
                    }

                    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
                    where
                        E: serde::de::Error,
                    {
                        Self::Value::from_b58check(v).map_err(|e| {
                            E::custom(format!("error constructing hash from base58check: {}", e))
                        })
                    }

                    fn visit_newtype_struct<E>(self, e: E) -> Result<Self::Value, E::Error>
                    where
                        E: serde::Deserializer<'de>,
                    {
                        let field0: Vec<u8> = match <Vec<u8> as serde::Deserialize>::deserialize(e)
                        {
                            Ok(val) => val,
                            Err(err) => {
                                return Err(err);
                            }
                        };
                        Ok($name(field0))
                    }

                    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                    where
                        A: serde::de::SeqAccess<'de>,
                    {
                        let hash = match seq.next_element::<Vec<u8>>() {
                            Ok(Some(val)) => val,
                            Ok(None) => {
                                return Err(serde::de::Error::custom("no hash bytes".to_string()))
                            }
                            Err(err) => return Err(err),
                        };
                        Self::Value::try_from_bytes(&hash).map_err(|e| {
                            serde::de::Error::custom(format!(
                                "error constructing hash from bytes: {}",
                                e
                            ))
                        })
                    }
                }

                if deserializer.is_human_readable() {
                    deserializer.deserialize_str(HashVisitor)
                } else {
                    deserializer.deserialize_newtype_struct(stringify!($name), HashVisitor)
                }
            }
        }
    };
}

define_hash!(ChainId);
define_hash!(BlockHash);
define_hash!(BlockMetadataHash);
define_hash!(OperationHash);
define_hash!(OperationListListHash);
define_hash!(OperationMetadataHash);
define_hash!(OperationMetadataListListHash);
define_hash!(ContextHash);
define_hash!(ProtocolHash);
define_hash!(ContractKt1Hash);
define_hash!(ContractTz1Hash);
define_hash!(ContractTz2Hash);
define_hash!(ContractTz3Hash);
define_hash!(CryptoboxPublicKeyHash);
define_hash!(PublicKeyEd25519);
define_hash!(PublicKeySecp256k1);
define_hash!(PublicKeyP256);
define_hash!(Ed25519Signature);
define_hash!(Signature);

/// Note: see Tezos ocaml lib_crypto/base58.ml
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum HashType {
    // "\087\082\000" (* Net(15) *)
    ChainId,
    // "\001\052" (* B(51) *)
    BlockHash,
    // "\234\249" (* bm(52) *)
    BlockMetadataHash,
    // "\002\170" (* P(51) *)
    ProtocolHash,
    // "\079\199" (* Co(52) *)
    ContextHash,
    // "\005\116" (* o(51) *)
    OperationHash,
    // "\029\159\109" (* LLo(53) *)
    OperationListListHash,
    // "\005\183" (* r(51) *)
    OperationMetadataHash,
    // "\029\159\182" (* LLr(53) *)
    OperationMetadataListListHash,
    // "\153\103" (* id(30) *)
    CryptoboxPublicKeyHash,
    // "\002\090\121" (* KT1(36) *)
    ContractKt1Hash,
    // "\006\161\159" (* tz1(36) *)
    ContractTz1Hash,
    // "\006\161\161" (* tz2(36) *)
    ContractTz2Hash,
    // "\006\161\164" (* tz3(36) *)
    ContractTz3Hash,
    // "\013\015\037\217" (* edpk(54) *)
    PublicKeyEd25519,
    // "\003\254\226\086" (* sppk(55) *)
    PublicKeySecp256k1,
    // "\003\178\139\127" (* p2pk(55) *)
    PublicKeyP256,
    // "\009\245\205\134\018" (* edsig(99) *)
    Ed25519Signature,
    // "\004\130\043" (* sig(96) *)
    Signature,
}

impl HashType {
    #[inline]
    pub fn base58check_prefix(&self) -> &'static [u8] {
        use prefix_bytes::*;
        match self {
            HashType::ChainId => &CHAIN_ID,
            HashType::BlockHash => &BLOCK_HASH,
            HashType::BlockMetadataHash => &BLOCK_METADATA_HASH,
            HashType::ContextHash => &CONTEXT_HASH,
            HashType::ProtocolHash => &PROTOCOL_HASH,
            HashType::OperationHash => &OPERATION_HASH,
            HashType::OperationListListHash => &OPERATION_LIST_LIST_HASH,
            HashType::OperationMetadataHash => &OPERATION_METADATA_HASH,
            HashType::OperationMetadataListListHash => &OPERATION_METADATA_LIST_LIST_HASH,
            HashType::CryptoboxPublicKeyHash => &CRYPTOBOX_PUBLIC_KEY_HASH,
            HashType::ContractKt1Hash => &CONTRACT_KT1_HASH,
            HashType::ContractTz1Hash => &CONTRACT_TZ1_HASH,
            HashType::ContractTz2Hash => &CONTRACT_TZ2_HASH,
            HashType::ContractTz3Hash => &CONTRACT_TZ3_HASH,
            HashType::PublicKeyEd25519 => &PUBLIC_KEY_ED25519,
            HashType::PublicKeySecp256k1 => &PUBLIC_KEY_SECP256K1,
            HashType::PublicKeyP256 => &PUBLIC_KEY_P256,
            HashType::Ed25519Signature => &ED22519_SIGNATURE_HASH,
            HashType::Signature => &GENERIC_SIGNATURE_HASH,
        }
    }

    /// Size of hash in bytes
    pub const fn size(&self) -> usize {
        match self {
            HashType::ChainId => 4,
            HashType::BlockHash
            | HashType::BlockMetadataHash
            | HashType::ContextHash
            | HashType::ProtocolHash
            | HashType::OperationHash
            | HashType::OperationListListHash
            | HashType::OperationMetadataHash
            | HashType::OperationMetadataListListHash
            | HashType::PublicKeyEd25519 => 32,
            HashType::CryptoboxPublicKeyHash => 16,
            HashType::ContractKt1Hash
            | HashType::ContractTz1Hash
            | HashType::ContractTz2Hash
            | HashType::ContractTz3Hash => 20,
            HashType::PublicKeySecp256k1 | HashType::PublicKeyP256 => 33,
            HashType::Ed25519Signature | HashType::Signature => 64,
        }
    }

    /// Convert hash byte representation into string.
    pub fn hash_to_b58check(&self, data: &[u8]) -> Result<String, FromBytesError> {
        if self.size() != data.len() {
            Err(FromBytesError::InvalidSize)
        } else {
            let mut hash = Vec::with_capacity(self.base58check_prefix().len() + data.len());
            if matches!(self, Self::Signature) && data == [0; Self::Ed25519Signature.size()] {
                hash.extend(Self::Ed25519Signature.base58check_prefix());
            } else {
                hash.extend(self.base58check_prefix());
            }
            hash.extend(data);
            hash.to_base58check()
                // currently the error is returned if the input lenght exceeds 128 bytes
                // that is the limitation of base58 crate, and there are no hash types
                // exceeding that limit.
                // TODO: remove this when TE-373 is implemented
                .map_err(|_| unreachable!("Hash size should not exceed allowed 128 bytes"))
        }
    }

    /// Convert string representation of the hash to bytes form.
    pub fn b58check_to_hash(&self, data: &str) -> Result<Hash, FromBase58CheckError> {
        let mut hash = data.from_base58check()?;
        if let HashType::Signature = self {
            // zero signature is represented as Ed25519 signature
            if hash.len()
                == HashType::Ed25519Signature.size()
                    + HashType::Ed25519Signature.base58check_prefix().len()
            {
                let (prefix, hash) =
                    hash.split_at(HashType::Ed25519Signature.base58check_prefix().len());
                if prefix == HashType::Ed25519Signature.base58check_prefix()
                    && hash == [0; HashType::Ed25519Signature.size()]
                {
                    return Ok(hash.to_vec());
                }
            }
        }
        let expected_len = self.size() + self.base58check_prefix().len();
        if expected_len != hash.len() {
            return Err(FromBase58CheckError::MismatchedLength {
                expected: expected_len,
                actual: hash.len(),
            });
        }
        // prefix is not present in a binary representation
        hash.drain(0..self.base58check_prefix().len());
        Ok(hash)
    }
}

/// Implementation of chain_id.ml -> of_block_hash
#[inline]
pub fn chain_id_from_block_hash(block_hash: &BlockHash) -> Result<ChainId, Blake2bError> {
    let result = crate::blake2b::digest_256(&block_hash.0)?;
    Ok(ChainId::from_bytes(&result[0..HashType::ChainId.size()])
        .unwrap_or_else(|_| unreachable!("ChainId is created from slice of correct size")))
}

#[derive(Debug, Clone, Error, serde::Serialize, serde::Deserialize)]
pub enum TryFromPKError {
    #[error("Error calculating digest")]
    Digest(#[from] Blake2bError),
    #[error("Invalid hash size")]
    Size(#[from] FromBytesError),
}

impl TryFrom<PublicKeyEd25519> for ContractTz1Hash {
    type Error = TryFromPKError;

    fn try_from(source: PublicKeyEd25519) -> Result<Self, Self::Error> {
        let hash = blake2b::digest_160(&source.0)?;
        let typed_hash = Self::from_bytes(&hash)?;
        Ok(typed_hash)
    }
}

impl TryFrom<PublicKeySecp256k1> for ContractTz2Hash {
    type Error = TryFromPKError;

    fn try_from(source: PublicKeySecp256k1) -> Result<Self, Self::Error> {
        let hash = blake2b::digest_160(&source.0)?;
        let typed_hash = Self::from_bytes(&hash)?;
        Ok(typed_hash)
    }
}

impl TryFrom<PublicKeyP256> for ContractTz3Hash {
    type Error = TryFromPKError;

    fn try_from(source: PublicKeyP256) -> Result<Self, Self::Error> {
        let hash = blake2b::digest_160(&source.0)?;
        let typed_hash = Self::from_bytes(&hash)?;
        Ok(typed_hash)
    }
}

impl TryFrom<&PublicKeyEd25519> for sodiumoxide::crypto::sign::PublicKey {
    type Error = FromBytesError;

    fn try_from(source: &PublicKeyEd25519) -> Result<Self, Self::Error> {
        Ok(sodiumoxide::crypto::sign::PublicKey(
            source
                .0
                .as_slice()
                .try_into()
                .map_err(|_| FromBytesError::InvalidSize)?,
        ))
    }
}

impl TryFrom<&Signature> for sodiumoxide::crypto::sign::Signature {
    type Error = FromBytesError;

    fn try_from(source: &Signature) -> Result<Self, Self::Error> {
        Ok(sodiumoxide::crypto::sign::Signature(
            source
                .0
                .as_slice()
                .try_into()
                .map_err(|_| FromBytesError::InvalidSize)?,
        ))
    }
}

impl PublicKeySignatureVerifier for PublicKeyEd25519 {
    type Signature = Signature;
    type Error = CryptoError;

    /// Verifies the correctness of `bytes` signed by Ed25519 as the `signature`.
    fn verify_signature(&self, signature: &Signature, bytes: &[u8]) -> Result<bool, Self::Error> {
        Ok(sodiumoxide::crypto::sign::verify_detached(
            &signature
                .try_into()
                .map_err(|_| CryptoError::InvalidSignature)?,
            bytes,
            &self.try_into().map_err(|_| CryptoError::InvalidPublicKey)?,
        ))
    }
}

impl PublicKeySignatureVerifier for PublicKeySecp256k1 {
    type Signature = Signature;
    type Error = CryptoError;

    /// Verifies the correctness of `bytes` signed by Secp256k1 as the `signature`.
    fn verify_signature(&self, signature: &Signature, bytes: &[u8]) -> Result<bool, Self::Error> {
        let pk = libsecp256k1::PublicKey::parse_slice(
            &self.0,
            Some(libsecp256k1::PublicKeyFormat::Compressed),
        )
        .map_err(|_| CryptoError::InvalidPublicKey)?;
        let sig = libsecp256k1::Signature::parse_standard_slice(&signature.0)
            .map_err(|_| CryptoError::InvalidSignature)?;
        let msg =
            libsecp256k1::Message::parse_slice(bytes).map_err(|_| CryptoError::InvalidMessage)?;

        Ok(libsecp256k1::verify(&msg, &sig, &pk))
    }
}

impl PublicKeySignatureVerifier for PublicKeyP256 {
    type Signature = Signature;
    type Error = CryptoError;

    /// Verifies the correctness of `bytes` signed by P256 as the `signature`.
    fn verify_signature(&self, signature: &Signature, bytes: &[u8]) -> Result<bool, Self::Error> {
        use p256::{
            ecdsa::signature::{
                digest::{FixedOutput, Reset, Update},
                DigestVerifier,
            },
            elliptic_curve::consts::U32,
        };

        // By default p256 crate uses sha256 to get a 32-bit hash from input message.
        // Here though, the input data is already a Tezos hash of proper size.
        // So we need to use identity digest.
        #[derive(Default, Clone)]
        struct NoHash([u8; CRYPTO_KEY_SIZE]);

        impl Update for NoHash {
            fn update(&mut self, data: impl AsRef<[u8]>) {
                let data = data.as_ref();
                let end = std::cmp::min(data.len(), self.0.len());
                self.0[..end].copy_from_slice(&data[..end]);
            }
        }

        impl FixedOutput for NoHash {
            type OutputSize = U32;

            fn finalize_into(
                self,
                out: &mut p256::elliptic_curve::generic_array::GenericArray<u8, Self::OutputSize>,
            ) {
                out.copy_from_slice(&self.0[..]);
            }

            fn finalize_into_reset(
                &mut self,
                out: &mut p256::elliptic_curve::generic_array::GenericArray<u8, Self::OutputSize>,
            ) {
                out.copy_from_slice(&self.0[..]);
            }
        }

        impl Reset for NoHash {
            fn reset(&mut self) {}
        }

        let pk = p256::ecdsa::VerifyingKey::from_sec1_bytes(&self.0)
            .map_err(|_| CryptoError::InvalidPublicKey)?;
        let r: [u8; 32] = signature.0[..32]
            .try_into()
            .map_err(|_| CryptoError::InvalidSignature)?;
        let s: [u8; 32] = signature.0[32..]
            .try_into()
            .map_err(|_| CryptoError::InvalidSignature)?;
        let sig = p256::ecdsa::Signature::from_scalars(r, s)
            .map_err(|_| CryptoError::InvalidSignature)?;
        Ok(pk
            .verify_digest(NoHash::default().chain(bytes), &sig)
            .map(|_| true)
            .unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto_box::PublicKey;
    use hex::FromHex;

    #[test]
    fn test_encode_chain_id() -> Result<(), anyhow::Error> {
        let decoded = HashType::ChainId.hash_to_b58check(&hex::decode("8eceda2f")?)?;
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_chain_id_to_b58_string() -> Result<(), anyhow::Error> {
        let encoded = &ChainId::from_bytes(&hex::decode("8eceda2f")?)?.to_base58_check();
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_chain_id_to_base58_check() -> Result<(), anyhow::Error> {
        let encoded = ChainId::from_bytes(&hex::decode("8eceda2f")?)?.to_base58_check();
        let expected = "NetXgtSLGNJvNye";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_chain_id_from_block_hash() -> Result<(), anyhow::Error> {
        let decoded_chain_id: ChainId = chain_id_from_block_hash(&BlockHash::from_base58_check(
            "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe",
        )?)?;
        let decoded_chain_id: &str = &decoded_chain_id.to_base58_check();
        let expected_chain_id = "NetXgtSLGNJvNye";
        assert_eq!(expected_chain_id, decoded_chain_id);

        let decoded_chain_id: ChainId = chain_id_from_block_hash(&BlockHash::from_base58_check(
            "BLockGenesisGenesisGenesisGenesisGenesisd6f5afWyME7",
        )?)?;
        let decoded_chain_id: &str = &decoded_chain_id.to_base58_check();
        let expected_chain_id = "NetXjD3HPJJjmcd";
        assert_eq!(expected_chain_id, decoded_chain_id);

        Ok(())
    }

    #[test]
    fn test_encode_block_header_genesis() -> Result<(), anyhow::Error> {
        let encoded = HashType::BlockHash.hash_to_b58check(&hex::decode(
            "8fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424affa610d",
        )?)?;
        let expected = "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header_genesis_new() -> Result<(), anyhow::Error> {
        let encoded = BlockHash::from_bytes(&hex::decode(
            "8fcf233671b6a04fcf679d2a381c2544ea6c1ea29ba6157776ed8424affa610d",
        )?)?
        .to_base58_check();
        let expected = "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header() -> Result<(), anyhow::Error> {
        let encoded = HashType::BlockHash.hash_to_b58check(&hex::decode(
            "46a6aefde9243ae18b191a8d010b7237d5130b3530ce5d1f60457411b2fa632d",
        )?)?;
        let expected = "BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_encode_block_header_new() -> Result<(), anyhow::Error> {
        let encoded = BlockHash::from_bytes(&hex::decode(
            "46a6aefde9243ae18b191a8d010b7237d5130b3530ce5d1f60457411b2fa632d",
        )?)?
        .to_base58_check();
        let expected = "BLFQ2JjYWHC95Db21cRZC4cgyA1mcXmx1Eg6jKywWy9b8xLzyK9";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_encode_context() -> Result<(), anyhow::Error> {
        let decoded = HashType::ContextHash.hash_to_b58check(&hex::decode(
            "934484026d24be9ad40c98341c20e51092dd62bbf470bb9ff85061fa981ebbd9",
        )?)?;
        let expected = "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_context_new() -> Result<(), anyhow::Error> {
        let encoded = ContextHash::from_bytes(&hex::decode(
            "934484026d24be9ad40c98341c20e51092dd62bbf470bb9ff85061fa981ebbd9",
        )?)?
        .to_base58_check();
        let expected = "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_encode_operations_hash() -> Result<(), anyhow::Error> {
        let decoded = HashType::OperationListListHash.hash_to_b58check(&hex::decode(
            "acecbfac449678f1d68b90c7b7a86c9280fd373d872e072f3fb1b395681e7149",
        )?)?;
        let expected = "LLoads9N8uB8v659hpNhpbrLzuzLdUCjz5euiR6Lm2hd7C6sS2Vep";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_operations_hash_new() -> Result<(), anyhow::Error> {
        let encoded = OperationListListHash::from_bytes(&hex::decode(
            "acecbfac449678f1d68b90c7b7a86c9280fd373d872e072f3fb1b395681e7149",
        )?)?
        .to_base58_check();
        let expected = "LLoads9N8uB8v659hpNhpbrLzuzLdUCjz5euiR6Lm2hd7C6sS2Vep";
        assert_eq!(expected, encoded);

        Ok(())
    }

    #[test]
    fn test_encode_public_key_hash() -> Result<(), anyhow::Error> {
        let pk = PublicKey::from_hex(
            "2cc1b580f4b8b1f6dbd0aa1d9cde2655c2081c07d7e61249aad8b11d954fb01a",
        )?;
        let pk_hash = pk.public_key_hash()?;
        let decoded = HashType::CryptoboxPublicKeyHash.hash_to_b58check(pk_hash.as_ref())?;
        let expected = "idsg2wkkDDv2cbEMK4zH49fjgyn7XT";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_public_key_hash_new() -> Result<(), anyhow::Error> {
        let pk = PublicKey::from_hex(
            "2cc1b580f4b8b1f6dbd0aa1d9cde2655c2081c07d7e61249aad8b11d954fb01a",
        )?;
        let pk_hash = pk.public_key_hash()?;
        let decoded = CryptoboxPublicKeyHash::from_bytes(pk_hash.as_ref())?.to_base58_check();
        let expected = "idsg2wkkDDv2cbEMK4zH49fjgyn7XT";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz1() -> Result<(), anyhow::Error> {
        let decoded = HashType::ContractTz1Hash
            .hash_to_b58check(&hex::decode("83846eddd5d3c5ed96e962506253958649c84a74")?)?;
        let expected = "tz1XdRrrqrMfsFKA8iuw53xHzug9ipr6MuHq";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz1_new() -> Result<(), anyhow::Error> {
        let decoded =
            ContractTz1Hash::from_bytes(&hex::decode("83846eddd5d3c5ed96e962506253958649c84a74")?)?
                .to_base58_check();
        let expected = "tz1XdRrrqrMfsFKA8iuw53xHzug9ipr6MuHq";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz2() -> Result<(), anyhow::Error> {
        let decoded = HashType::ContractTz2Hash
            .hash_to_b58check(&hex::decode("2fcb1d9307f0b1f94c048ff586c09f46614c7e90")?)?;
        let expected = "tz2Cfwk4ortcaqAGcVJKSxLiAdcFxXBLBoyY";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_tz3() -> Result<(), anyhow::Error> {
        let decoded = HashType::ContractTz3Hash
            .hash_to_b58check(&hex::decode("193b2b3f6b8f8e1e6b39b4d442fc2b432f6427a8")?)?;
        let expected = "tz3NdTPb3Ax2rVW2Kq9QEdzfYFkRwhrQRPhX";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_encode_contract_kt1() -> Result<(), anyhow::Error> {
        let decoded = HashType::ContractKt1Hash
            .hash_to_b58check(&hex::decode("42b419240509ddacd12839700b7f720b4aa55e4e")?)?;
        let expected = "KT1EfTusMLoeCAAGd9MZJn5yKzFr6kJU5U91";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_block_header_hash() -> Result<(), anyhow::Error> {
        let decoded = HashType::BlockHash
            .b58check_to_hash("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?;
        let decoded = hex::encode(&decoded);
        let expected = "2253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_operations_hash() -> Result<(), anyhow::Error> {
        let decoded = HashType::OperationListListHash
            .b58check_to_hash("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc")?;
        let decoded = hex::encode(&decoded);
        let expected = "7c09f7c4d76ace86e1a7e1c7dc0a0c7edcaa8b284949320081131976a87760c3";
        assert_eq!(expected, decoded);

        Ok(())
    }

    #[test]
    fn test_decode_protocol_hash() -> Result<(), anyhow::Error> {
        let decoded =
            ProtocolHash::from_base58_check("PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb")?;
        let decoded = hex::encode(decoded.as_ref());
        let expected = "3e5e3a606afab74a59ca09e333633e2770b6492c5e594455b71e9a2f0ea92afb";
        assert_eq!(expected, decoded);

        assert_eq!(
            "PsCARTHAGazKbHtnKfLzQg3kms52kSRpgnDY982a9oYsSXRLQEb",
            HashType::ProtocolHash.hash_to_b58check(&hex::decode(decoded)?)?
        );
        Ok(())
    }

    #[test]
    fn test_decode_block_metadata_hash() -> Result<(), anyhow::Error> {
        let decoded = HashType::BlockMetadataHash
            .b58check_to_hash("bm2gU1qwmoPNsXzFKydPDHWX37es6C5Z4nHyuesW8YxbkZ1339cN")?;
        let decoded = hex::encode(&decoded);
        let expected = "0e5751c026e543b2e8ab2eb06099daa1d1e5df47778f7787faab45cdf12fe3a8";
        assert_eq!(expected, decoded);

        assert_eq!(
            "bm2gU1qwmoPNsXzFKydPDHWX37es6C5Z4nHyuesW8YxbkZ1339cN",
            HashType::BlockMetadataHash.hash_to_b58check(&hex::decode(decoded)?)?
        );
        Ok(())
    }

    #[test]
    fn test_decode_operation_metadata_list_list_hash() -> Result<(), anyhow::Error> {
        let decoded = HashType::OperationMetadataListListHash
            .b58check_to_hash("LLr283rR7AWhepNeHcP9msa2VeAurWtodBLrnSjwaxpNyiyfhYcKX")?;
        let decoded = hex::encode(&decoded);
        let expected = "761223dee6643bb9f28acf45f2a44ae1a3e2fd68bfc1a00e3f539cc2dc637632";
        assert_eq!(expected, decoded);

        assert_eq!(
            "LLr283rR7AWhepNeHcP9msa2VeAurWtodBLrnSjwaxpNyiyfhYcKX",
            HashType::OperationMetadataListListHash.hash_to_b58check(&hex::decode(decoded)?)?
        );
        Ok(())
    }

    #[test]
    fn test_decode_operation_metadata_hash() -> Result<(), anyhow::Error> {
        let decoded = HashType::OperationMetadataHash
            .b58check_to_hash("r3E9xb2QxUeG56eujC66B56CV8mpwjwfdVmEpYu3FRtuEx9tyfG")?;
        let decoded = hex::encode(&decoded);
        let expected = "2d905a5c4fefad1f1ab8a5436f26b15290f2fe2bea111c85ee4626156dc6b4da";
        assert_eq!(expected, decoded);

        assert_eq!(
            "r3E9xb2QxUeG56eujC66B56CV8mpwjwfdVmEpYu3FRtuEx9tyfG",
            HashType::OperationMetadataHash.hash_to_b58check(&hex::decode(decoded)?)?
        );
        Ok(())
    }

    #[test]
    fn test_b58_to_hash_mismatched_lenght() -> Result<(), anyhow::Error> {
        let b58 = HashType::ChainId.hash_to_b58check(&[0, 0, 0, 0])?;
        let result = HashType::BlockHash.b58check_to_hash(&b58);
        assert!(matches!(
            result,
            Err(FromBase58CheckError::MismatchedLength {
                expected: _,
                actual: _
            })
        ));
        Ok(())
    }

    #[test]
    fn test_b85_to_signature_hash() -> Result<(), anyhow::Error> {
        let encoded = "sigbQ5ZNvkjvGssJgoAnUAfY4Wvvg3QZqawBYB1j1VDBNTMBAALnCzRHWzer34bnfmzgHg3EvwdzQKdxgSghB897cono6gbQ";
        let decoded = hex::encode(HashType::Signature.b58check_to_hash(encoded)?);
        let expected = "66804fe735e06e97e26da8236b6341b91c625d5e82b3524ec0a88cc982365e70f8a5b9bc65df2ea6d21ee244cc3a96fb33031c394c78b1179ff1b8a44237740c";
        assert_eq!(expected, decoded);

        assert_eq!(
            encoded,
            HashType::Signature.hash_to_b58check(&hex::decode(decoded)?)?
        );
        Ok(())
    }

    #[test]
    fn test_ed255519_signature_verification() {
        let pk = PublicKeyEd25519::from_base58_check(
            "edpkvWR5truf7AMF3PZVCXx7ieQLCW4MpNDzM3VwPfmFWVbBZwswBw",
        )
        .unwrap();
        let sig = Signature::from_base58_check(
            "sigdGBG68q2vskMuac4AzyNb1xCJTfuU8MiMbQtmZLUCYydYrtTd5Lessn1EFLTDJzjXoYxRasZxXbx6tHnirbEJtikcMHt3"
        ).unwrap();
        let msg = hex::decode("bcbb7b77cb0712e4cd02160308cfd53e8dde8a7980c4ff28b62deb12304913c2")
            .unwrap();

        let result = pk.verify_signature(&sig, &msg).unwrap();
        assert!(result);
    }

    #[test]
    fn test_secp256k1_signature_verification() {
        let pk = PublicKeySecp256k1::from_base58_check(
            "sppk7cwkTzCPptCSxSTvGNg4uqVcuTbyWooLnJp4yxJNH5DReUGxYvs",
        )
        .unwrap();
        let sig = Signature::from_base58_check("sigrJ2jqanLupARzKGvzWgL1Lv6NGUqDovHKQg9MX4PtNtHXgcvG6131MRVzujJEXfvgbuRtfdGbXTFaYJJjuUVLNNZTf5q1").unwrap();
        let msg = hex::decode("5538e2cc90c9b053a12e2d2f3a985aff1809eac59501db4d644e4bb381b06b4b")
            .unwrap();

        let result = pk.verify_signature(&sig, &msg).unwrap();
        assert!(result);
    }

    #[test]
    fn test_p256_signature_verification() {
        let pk = PublicKeyP256::from_base58_check(
            "p2pk67Cwb5Ke6oSmqeUbJxURXMe3coVnH9tqPiB2xD84CYhHbBKs4oM",
        )
        .unwrap();
        let sig = Signature::from_base58_check(
            "sigNCaj9CnmD94eZH9C7aPPqBbVCJF72fYmCFAXqEbWfqE633WNFWYQJFnDUFgRUQXR8fQ5tKSfJeTe6UAi75eTzzQf7AEc1"
        ).unwrap();
        let msg = hex::decode("5538e2cc90c9b053a12e2d2f3a985aff1809eac59501db4d644e4bb381b06b4b")
            .unwrap();

        let result = pk.verify_signature(&sig, &msg).unwrap();
        assert!(result);
    }

    mod hash_as_json_is_base58check {
        use super::super::*;

        macro_rules! test {
            ($name:ident, $ty:ident, $h:expr) => {
                #[test]
                fn $name() {
                    for str in $h {
                        let h = $ty::from_base58_check(str).expect("Invalid hash");
                        let json = serde_json::to_string(&h).expect("Cannot convert to json");
                        assert_eq!(json, format!(r#""{}""#, h));
                        let h1 = serde_json::from_str(&json).expect("Cannot convert from json");
                        assert_eq!(h, h1);
                    }
                }
            };
        }

        test!(chain_id, ChainId, ["NetXZSsxBpMQeAT"]);

        test!(
            block_hash,
            BlockHash,
            [
                "BLockGenesisGenesisGenesisGenesisGenesis7e8c4d4snJW",
                "BLBok16TeLoQijYkCkWec33HMi5mMfZM8xTrxFd7KTgmtHPdptc"
            ]
        );

        test!(
            block_metadata_hash,
            BlockMetadataHash,
            ["bm2gU1qwmoPNsXzFKydPDHWX37es6C5Z4nHyuesW8YxbkZ1339cN"]
        );

        test!(
            operation_hash,
            OperationHash,
            ["ooZA4Y7nqiACwGwB53umSsKjobFrz7NYCo3TbQEkSNQsXuDVqkm"]
        );

        test!(
            operation_list_list_hash,
            OperationListListHash,
            ["LLoZqBDX1E2ADRXbmwYo8VtMNeHG6Ygzmm4Zqv97i91UPBQHy9Vq3"]
        );

        test!(operation_metadata_hash, OperationMetadataHash, []);

        test!(
            operation_metadata_list_list_hash,
            OperationMetadataListListHash,
            []
        );

        test!(
            context_hash,
            ContextHash,
            ["CoVDyf9y9gHfAkPWofBJffo4X4bWjmehH2LeVonDcCKKzyQYwqdk"]
        );

        test!(
            protocol_hash,
            ProtocolHash,
            [
                "PrihK96nBAFSxVL1GLJTVhu9YnzkMFiBeuJRPA8NwuZVZCE1L6i",
                "PtHangz2aRngywmSRGGvrcTyMbbdpWdpFKuS4uMWxg2RaH9i1qx"
            ]
        );

        test!(kt1_hash, ContractKt1Hash, []);

        test!(tz1_hash, ContractTz1Hash, []);

        test!(tz2_hash, ContractTz2Hash, []);

        test!(tz3_hash, ContractTz3Hash, []);

        test!(pk_hash, CryptoboxPublicKeyHash, []);

        test!(pk_ed25519, PublicKeyEd25519, []);

        test!(pk_secp256k1, PublicKeySecp256k1, []);

        test!(pk_p256, PublicKeyP256, []);

        test!(ed25519_sig, Ed25519Signature, ["edsigtXomBKi5CTRf5cjATJWSyaRvhfYNHqSUGrn4SdbYRcGwQrUGjzEfQDTuqHhuA8b2d8NarZjz8TRf65WkpQmo423BtomS8Q"]);

        test!(generic_sig, Signature, ["sigNCaj9CnmD94eZH9C7aPPqBbVCJF72fYmCFAXqEbWfqE633WNFWYQJFnDUFgRUQXR8fQ5tKSfJeTe6UAi75eTzzQf7AEc1"]);
    }
}
