// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Defines types of the intermediate data format.

use crate::encoding::Encoding;
use crate::encoding::HasEncoding;
use crate::has_encoding;

use serde::{Deserialize, Serialize};

#[cfg(fuzzing)]
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
#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Mutez(#[cfg_attr(fuzzing, field_mutator(BigIntMutator))] pub num_bigint::BigInt);

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
