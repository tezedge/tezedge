// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Defines types of the intermediate data format.

use std::str::FromStr;

use crate::enc::BinWriter;
use crate::encoding::Encoding;
use crate::encoding::HasEncoding;
use crate::has_encoding;
use crate::nom::NomReader;

use hex::FromHexError;
use num_bigint::Sign;
use serde::{Deserialize, Serialize};

#[cfg(feature = "fuzzing")]
use crate::fuzzing::bigint::BigIntMutator;

/// This is a wrapper for [num_bigint::BigInt] type.
#[derive(PartialEq, Debug, Clone)]
pub struct BigInt(pub num_bigint::BigInt);

impl From<num_bigint::BigInt> for BigInt {
    fn from(from: num_bigint::BigInt) -> Self {
        BigInt(from)
    }
}

impl From<BigInt> for num_bigint::BigInt {
    fn from(from: BigInt) -> Self {
        from.0
    }
}

impl From<&num_bigint::BigInt> for BigInt {
    fn from(from: &num_bigint::BigInt) -> Self {
        BigInt(from.clone())
    }
}

impl From<&BigInt> for num_bigint::BigInt {
    fn from(from: &BigInt) -> Self {
        from.0.clone()
    }
}

/// Zarith number
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Zarith(pub num_bigint::BigInt);

impl From<num_bigint::BigInt> for Zarith {
    fn from(from: num_bigint::BigInt) -> Self {
        Zarith(from)
    }
}

impl From<Zarith> for num_bigint::BigInt {
    fn from(from: Zarith) -> Self {
        from.0
    }
}

impl From<&num_bigint::BigInt> for Zarith {
    fn from(from: &num_bigint::BigInt) -> Self {
        Zarith(from.clone())
    }
}

impl From<&Zarith> for num_bigint::BigInt {
    fn from(from: &Zarith) -> Self {
        from.0.clone()
    }
}

impl From<Zarith> for BigInt {
    fn from(source: Zarith) -> Self {
        Self(source.0)
    }
}

impl From<&Zarith> for BigInt {
    fn from(source: &Zarith) -> Self {
        Self(source.0.clone())
    }
}

has_encoding!(Zarith, ZARITH_ENCODING, { Encoding::Z });

/// Mutez number
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug)]
pub struct Mutez(
    #[cfg_attr(feature = "fuzzing", field_mutator(BigIntMutator))] pub num_bigint::BigInt,
);

impl<'de> Deserialize<'de> for Mutez {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let string: String = serde::Deserialize::deserialize(deserializer)?;
            let big_int: num_bigint::BigInt = string
                .parse()
                .map_err(|err| serde::de::Error::custom(format!("cannot parse big int: {err}")))?;
            if big_int.sign() == Sign::Minus {
                return Err(serde::de::Error::custom("negative number for natural"));
            }
            Ok(Self(big_int))
        } else {
            Ok(Self(serde::Deserialize::deserialize(deserializer)?))
        }
    }
}

impl Serialize for Mutez {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            let string = self.0.to_string();
            string.serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl From<num_bigint::BigInt> for Mutez {
    fn from(from: num_bigint::BigInt) -> Self {
        Mutez(from)
    }
}

impl From<Mutez> for num_bigint::BigInt {
    fn from(from: Mutez) -> Self {
        from.0
    }
}

impl From<&num_bigint::BigInt> for Mutez {
    fn from(from: &num_bigint::BigInt) -> Self {
        Mutez(from.clone())
    }
}

impl From<&Mutez> for num_bigint::BigInt {
    fn from(from: &Mutez) -> Self {
        from.0.clone()
    }
}

impl From<Mutez> for BigInt {
    fn from(source: Mutez) -> Self {
        Self(source.0)
    }
}

impl From<&Mutez> for BigInt {
    fn from(source: &Mutez) -> Self {
        Self(source.0.clone())
    }
}

has_encoding!(Mutez, MUTEZ_ENCODING, { Encoding::Mutez });

#[derive(Clone, PartialEq, Eq)]
//#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct SizedBytes<const SIZE: usize>(pub [u8; SIZE]);

impl<const SIZE: usize> std::fmt::Display for SizedBytes<SIZE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0))
    }
}

impl<const SIZE: usize> std::fmt::Debug for SizedBytes<SIZE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bytes: {}", hex::encode(self.0))
    }
}

impl<const SIZE: usize> AsRef<[u8]> for SizedBytes<SIZE> {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const SIZE: usize> AsMut<[u8]> for SizedBytes<SIZE> {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl<const SIZE: usize> From<[u8; SIZE]> for SizedBytes<SIZE> {
    fn from(bytes: [u8; SIZE]) -> Self {
        Self(bytes)
    }
}

impl<const SIZE: usize> From<SizedBytes<SIZE>> for [u8; SIZE] {
    fn from(bytes: SizedBytes<SIZE>) -> Self {
        bytes.0
    }
}

impl<const SIZE: usize> Serialize for SizedBytes<SIZE> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            serializer.serialize_str(&hex::encode(self.0.as_ref()))
        } else {
            serializer.serialize_newtype_struct(stringify!(Bytes<SIZE>), self.0.as_ref())
        }
    }
}

struct BytesVisitor<const SIZE: usize>;

impl<'de, const SIZE: usize> serde::de::Visitor<'de> for BytesVisitor<SIZE> {
    type Value = SizedBytes<{ SIZE }>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("eigher sequence of bytes or hex encoded data expected")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let mut bytes = [0_u8; SIZE];
        hex::decode_to_slice(v, &mut bytes)
            .map_err(|e| E::custom(format!("error constructing bytes from hex: {e}")))?;
        Ok(bytes.into())
    }

    fn visit_newtype_struct<E>(self, e: E) -> Result<Self::Value, E::Error>
    where
        E: serde::Deserializer<'de>,
    {
        let mut bytes = [0_u8; SIZE];
        match <Vec<u8> as serde::Deserialize>::deserialize(e) {
            Ok(val) if val.len() == SIZE => bytes.copy_from_slice(&val),
            Ok(val) => {
                return Err(serde::de::Error::custom(format!(
                    "invalid lenght, expected {SIZE}, got {len}",
                    len = val.len()
                )))
            }
            Err(err) => {
                return Err(err);
            }
        };
        Ok(bytes.into())
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut bytes = [0_u8; SIZE];
        match seq.next_element::<Vec<u8>>() {
            Ok(Some(val)) if val.len() == SIZE => bytes.copy_from_slice(&val),
            Ok(Some(val)) => {
                return Err(serde::de::Error::custom(format!(
                    "invalid lenght, expected {SIZE}, got {len}",
                    len = val.len()
                )))
            }
            Ok(None) => return Err(serde::de::Error::custom("no bytes".to_string())),
            Err(err) => return Err(err),
        };
        Ok(bytes.into())
    }
}

impl<'de, const SIZE: usize> Deserialize<'de> for SizedBytes<SIZE> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            deserializer.deserialize_str(BytesVisitor)
        } else {
            deserializer.deserialize_newtype_struct("Bytes<SIZE>", BytesVisitor)
        }
    }
}

impl<const SIZE: usize> NomReader for SizedBytes<SIZE> {
    fn nom_read(input: &[u8]) -> crate::nom::NomResult<Self> {
        use crate::nom;
        let (input, slice) = nom::sized(SIZE, nom::bytes)(input)?;
        let mut bytes = [0; SIZE];
        bytes.copy_from_slice(&slice);
        Ok((input, bytes.into()))
    }
}

impl<const SIZE: usize> BinWriter for SizedBytes<SIZE> {
    fn bin_write(&self, bytes: &mut Vec<u8>) -> crate::enc::BinResult {
        use crate::enc;
        enc::put_bytes(&self.0, bytes);
        Ok(())
    }
}

impl<const SIZE: usize> HasEncoding for SizedBytes<SIZE> {
    fn encoding() -> Encoding {
        Encoding::sized(SIZE, Encoding::Bytes)
    }
}

/// Sequence of bytes bounded by maximum size
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Bytes(Vec<u8>);

#[derive(Debug, thiserror::Error)]
pub enum BytesDecodeError {
    #[error(transparent)]
    Hex(#[from] FromHexError),
}

impl Bytes {
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl std::fmt::Debug for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Bytes").field(&self.to_string()).finish()
    }
}

impl std::fmt::Display for Bytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        hex::encode(&self.0).fmt(f)
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(source: Vec<u8>) -> Self {
        Self(source)
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(source: Bytes) -> Self {
        source.0
    }
}

impl FromStr for Bytes {
    type Err = BytesDecodeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(hex::decode(s)?))
    }
}

impl AsRef<[u8]> for Bytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsRef<Vec<u8>> for Bytes {
    fn as_ref(&self) -> &Vec<u8> {
        &self.0
    }
}

impl HasEncoding for Bytes {
    fn encoding() -> Encoding {
        Encoding::list(Encoding::Uint8)
    }
}

impl NomReader for Bytes {
    fn nom_read(input: &[u8]) -> crate::nom::NomResult<Self> {
        use crate::nom::bytes;
        let (input, b) = bytes(input)?;
        Ok((input, Self(b)))
    }
}

impl BinWriter for Bytes {
    fn bin_write(&self, output: &mut Vec<u8>) -> crate::enc::BinResult {
        crate::enc::put_bytes(self.0.as_ref(), output);
        Ok(())
    }
}

impl serde::Serialize for Bytes {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            let hex_bytes = hex::encode(&self.0);
            serde::Serialize::serialize(&hex_bytes, serializer)
        } else {
            serde::Serialize::serialize(&self.0, serializer)
        }
    }
}

impl<'de> serde::Deserialize<'de> for Bytes {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let hex_bytes: String = serde::Deserialize::deserialize(deserializer)?;
            let bytes = hex::decode(&hex_bytes).map_err(|err| {
                serde::de::Error::custom(format!("error decoding from hex string: {err}"))
            })?;
            Ok(Self(bytes))
        } else {
            let bytes = serde::Deserialize::deserialize(deserializer)?;
            Ok(Self(bytes))
        }
    }
}

/// Represents `true` value in binary format.
pub const BYTE_VAL_TRUE: u8 = 0xFF;
/// Represents `false` value in binary format.
pub const BYTE_VAL_FALSE: u8 = 0;
/// Represents `Some(x)` value in binary format.
pub const BYTE_VAL_SOME: u8 = 1;
/// Represents `None` value in binary format.
pub const BYTE_VAL_NONE: u8 = 0;
/// TE-172 - Represents 'Optional field' in binary format.
pub const BYTE_FIELD_SOME: u8 = 0xFF;
/// TE-172 - Represents `None` for 'Optional field' in binary format.
pub const BYTE_FIELD_NONE: u8 = 0;

/// Represents data in the intermediate form.
///
/// Rust struct is first converted to this intermediate form is then used for produce binary or json output.
/// Also this intermediate form is used when rust type is being created from a binary or json input.
///
/// # How it works:
///
/// Imagine we have struct `MyStruct` we want to serialize.
/// ```rust
/// struct MyStruct {
///   count: i32,
///   diameter: f32
/// }
/// let my_struct = MyStruct { count: 1, diameter: 102.95 };
/// ```
///
/// First we need to convert it to intermediate form represented by the [Value] type.
/// Structure will be converted to:
/// ```rust
/// use crate::tezos_encoding::types::Value;
/// let intermediate = Value::Record(vec![
///     ("count".into(), Value::Int32(1)),
///     ("diameter".into(), Value::Float(102.95))
/// ]);
/// ```
///
/// After that the intermediate form can be converted to binary by passing it to [crate::binary_writer::BinaryWriter].
#[derive(PartialEq, Debug)]
pub enum Value {
    /// Nothing: data is omitted from binary.
    Unit,
    /// Signed 8 bit integer (data is encoded as a byte in binary and an integer in JSON).
    Int8(i8),
    /// Unsigned 8 bit integer (data is encoded as a byte in binary and an integer in JSON).
    Uint8(u8),
    /// Signed 16 bit integer (data is encoded as a short in binary and an integer in JSON).
    Int16(i16),
    /// Unsigned 16 bit integer (data is encoded as a short in binary and an integer in JSON).
    Uint16(u16),
    /// Signed 31 bit integer, which corresponds to type int on 32-bit OCaml systems (data is encoded as a 32 bit int in binary and an integer in JSON).
    Int31(i32),
    /// Signed 32 bit integer (data is encoded as a 32-bit int in binary and an integer in JSON).
    Int32(i32),
    /// Signed 64 bit integer (data is encoded as a 64-bit int in binary and a decimal string in JSON).
    Int64(i64),
    /// Integer with bounds in a given range. Both bounds are inclusive.
    RangedInt(i32),
    /// Encoding of floating point number (encoded as a floating point number in JSON and a double in binary).
    Float(f64),
    /// Float with bounds in a given range. Both bounds are inclusive.
    RangedFloat(f64),
    /// Encoding of a boolean (data is encoded as a byte in binary and a boolean in JSON).
    Bool(bool),
    /// Encoding of a string
    /// - encoded as a byte sequence in binary prefixed by the length
    /// of the string
    /// - encoded as a string in JSON.
    String(String),
    /// Encoding of arbitrary bytes (encoded via hex in JSON and directly as a sequence byte in binary).
    Bytes(Vec<u8>),
    /// Combinator to make an optional value
    /// (represented as a 1-byte tag followed by the data (or nothing) in binary
    ///  and either the raw value or an empty object in JSON).
    Option(Option<Box<Value>>),
    /// List combinator.
    /// - encoded as an array in JSON
    /// - encoded as the concatenation of all the element in binary
    /// in binary prefixed by its length in bytes
    List(Vec<Value>),
    /// Enum value with name and/or ordinal number
    Enum(Option<String>, Option<u32>),
    /// Tag value with variant id and tag inner value
    Tag(String, Box<Value>),
    /// A Record is represented by a vector of (`<record name>`, `value`).
    /// This allows schema-less encoding.
    ///
    /// See [Record](types.Record) for a more user-friendly support.
    Record(Vec<(String, Value)>),
    /// Tuple is heterogeneous collection of values, it should have fixed amount of elements
    Tuple(Vec<Value>),
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn bytes_to_string() {
        let bytes = Bytes(vec![0xde, 0xad, 0xbe, 0xef]);
        let bytes_hex = bytes.to_string();
        assert_eq!(&bytes_hex, "deadbeef");
    }

    #[test]
    fn bytes_parse() {
        let bytes: Bytes = "deadbeef".parse().unwrap();
        assert_eq!(bytes, Bytes(vec![0xde, 0xad, 0xbe, 0xef]));
    }

    #[test]
    fn bytes_parse_error() {
        let _ = "deadbeefe".parse::<Bytes>().expect_err("");
        let _ = "morebeef".parse::<Bytes>().expect_err("");
    }

    #[test]
    fn bytes_to_json() {
        let bytes = Bytes(vec![0xde, 0xad, 0xbe, 0xef]);
        let json = serde_json::to_value(bytes).unwrap();
        assert!(matches!(json.as_str(), Some("deadbeef")))
    }

    #[test]
    fn bytes_from_json() {
        let json = serde_json::json!("deadbeef");
        let bytes: Bytes = serde_json::from_value(json).unwrap();
        assert_eq!(bytes, Bytes(vec![0xde, 0xad, 0xbe, 0xef]));
    }
}
