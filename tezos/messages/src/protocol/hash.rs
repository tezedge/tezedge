// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::marker::PhantomData;

use crypto::{base58::FromBase58CheckError, hash::{FromBytesError, HashTrait, HashType}};
use tezos_encoding::{enc::BinWriter, encoding::HasEncoding, nom::NomReader};

#[derive(Debug, Clone, PartialEq, Hash)]
pub struct Hash<H>(pub H);

impl<H> From<H> for Hash<H> {
    fn from(source: H) -> Self {
        Self(source)
    }
}

impl<H> Hash<H>
where
    H: HashTrait,
{
    pub fn from_base58_check(data: &str) -> Result<Self, FromBase58CheckError> {
        H::from_b58check(data).map(Self)
    }

    pub fn to_base58_check(&self) -> String {
        self.0.to_b58check()
    }
}

impl<H> serde::Serialize for Hash<H>
where
    H: HashTrait,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_b58check())
    }
}

struct HashVisitor<H>(PhantomData<H>);

impl<H> Default for HashVisitor<H> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<'de, H> serde::de::Visitor<'de> for HashVisitor<H>
where
    H: HashTrait,
{
    type Value = H;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("base58 encoded data expected")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Self::Value::from_b58check(v)
            .map_err(|e| E::custom(format!("error constructing hash from base58check: {}", e)))
    }
}

impl<'de, H> serde::Deserialize<'de> for Hash<H>
where
    H: HashTrait,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        deserializer
            .deserialize_str(HashVisitor::default())
            .map(Self)
    }
}

impl<H> HasEncoding for Hash<H>
where
    H: HasEncoding,
{
    fn encoding() -> &'static tezos_encoding::encoding::Encoding {
        H::encoding()
    }
}

impl<H> NomReader for Hash<H>
where
    H: NomReader,
{
    fn nom_read(bytes: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
        nom::combinator::map(H::nom_read, Self)(bytes)
    }
}

impl<H> BinWriter for Hash<H>
where
    H: BinWriter,
{
    fn bin_write(&self, bytes: &mut Vec<u8>) -> tezos_encoding::enc::BinResult {
        self.0.bin_write(bytes)
    }
}

impl<H> From<Hash<H>> for Vec<u8>
where H: Into<Vec<u8>>
{
    fn from(hash: Hash<H>) -> Self {
        hash.0.into()
    }
}

impl<H> AsRef<Vec<u8>> for Hash<H>
where H: AsRef<Vec<u8>>
{
    fn as_ref(&self) -> &Vec<u8> {
        self.0.as_ref()
    }
}

impl<H> HashTrait for Hash<H>
where
    H: HashTrait
{
    /// Returns this hash type.
    fn hash_type() -> HashType {
        H::hash_type()
    }

    /// Returns the size of this hash.
    fn hash_size() -> usize {
        H::hash_size()
    }

    /// Tries to create this hash from the `bytes`.
    fn try_from_bytes(bytes: &[u8]) -> Result<Self, FromBytesError> {
        H::try_from_bytes(bytes).map(Self)
    }

    fn from_b58check(data: &str) -> Result<Self, FromBase58CheckError> {
        H::from_b58check(data).map(Self)
    }

    fn to_b58check(&self) -> String {
        self.0.to_b58check()
    }
}
