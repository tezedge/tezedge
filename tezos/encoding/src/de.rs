// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Serde Deserializer

use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::slice::Iter;

use serde::de::{self, Deserialize, DeserializeSeed, Error as _, IntoDeserializer, Visitor};
use serde::forward_to_deserialize_any;

use crate::binary_reader::BinaryReaderError;
use crate::encoding::Encoding;
use crate::types::{BigInt, Value};

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn encoding_mismatch(encoding: &Encoding, value: &Value) -> Self {
        Error {
            message: format!("Unsupported encoding {:?} for value: {:?}", encoding, value),
        }
    }
    pub fn unsupported_value<T: fmt::Display>(value: &T) -> Self {
        Error {
            message: format!("Unsupported value: {}", value),
        }
    }
}

impl de::Error for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error {
            message: msg.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_fmt(format_args!("{}", self))
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        &self.message
    }
}

impl From<io::Error> for Error {
    fn from(from: io::Error) -> Self {
        Error {
            message: format!("I/O error. Reason: {:?}", from),
        }
    }
}

impl From<hex::FromHexError> for Error {
    fn from(from: hex::FromHexError) -> Self {
        Error {
            message: format!("Error decoding hex value. Reason: {:?}", from),
        }
    }
}

pub struct Deserializer<'de> {
    input: &'de Value,
}

struct SeqDeserializer<'de> {
    input: Iter<'de, Value>,
}

struct StructDeserializer<'de> {
    input: Iter<'de, (String, Value)>,
    value: Option<&'de Value>,
}

impl<'de> Deserializer<'de> {
    pub fn new(input: &'de Value) -> Self {
        Deserializer { input }
    }
}

impl<'de> SeqDeserializer<'de> {
    pub fn new(input: &'de [Value]) -> Self {
        SeqDeserializer {
            input: input.iter(),
        }
    }
}

impl<'de> StructDeserializer<'de> {
    pub fn new(input: &'de [(String, Value)]) -> Self {
        StructDeserializer {
            input: input.iter(),
            value: None,
        }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer<'de> {
    type Error = Error;

    // Look at the input data to decide what Serde data model type to
    // deserialize as. Not all data formats are able to support this operation.
    // Formats that support `deserialize_any` are known as self-describing.
    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Unit => visitor.visit_unit(),
            Value::Bool(b) => visitor.visit_bool(b),
            Value::Int8(i) => visitor.visit_i8(i),
            Value::Uint8(i) => visitor.visit_u8(i),
            Value::Int16(i) => visitor.visit_i16(i),
            Value::Uint16(i) => visitor.visit_u16(i),
            Value::Int31(i) | Value::Int32(i) | Value::RangedInt(i) => visitor.visit_i32(i),
            Value::Int64(x) => visitor.visit_i64(x),
            Value::Float(x) => visitor.visit_f64(x),
            _ => Err(Error::custom(format!(
                "Unsupported value of type {:?} in deserialize_any.",
                self.input
            ))),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
    }

    // The `Serializer` implementation on the previous page serialized chars as
    // single-character strings so handle that representation here.
    fn deserialize_char<V>(self, _: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Error::custom("tezos protocol does not support char"))
    }

    // Refer to the "Understanding deserializer lifetimes" page for information
    // about the three deserialization flavors of strings in Serde.
    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_str(s),
            Value::Bytes(ref bytes) => ::std::str::from_utf8(bytes)
                .map_err(|e| Error::custom(e.to_string()))
                .and_then(|s| visitor.visit_str(s)),
            _ => Err(Error::custom(format!(
                "not a string|bytes|fixed but a {:?}",
                self.input
            ))),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_string(s.to_owned()),
            Value::Bytes(ref bytes) => String::from_utf8(bytes.to_owned())
                .map_err(|e| Error::custom(e.to_string()))
                .and_then(|s| visitor.visit_string(s)),
            _ => Err(Error::custom(format!(
                "not a string|bytes|fixed but a {:?}",
                self.input
            ))),
        }
    }

    // The `Serializer` implementation on the previous page serialized byte
    // arrays as JSON arrays of bytes. Handle that representation here.
    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_bytes(s.as_bytes()),
            Value::Bytes(ref bytes) => visitor.visit_bytes(bytes),
            _ => Err(Error::custom(format!(
                "not a string|bytes|fixed but a {:?}",
                self.input
            ))),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_byte_buf(s.clone().into_bytes()),
            Value::Bytes(ref bytes) => visitor.visit_byte_buf(bytes.to_owned()),
            _ => Err(Error::custom("not a string|bytes|fixed")),
        }
    }

    // An absent optional is represented as the JSON `null` and a present
    // optional is represented as just the contained value.
    //
    // As commented in `Serializer` implementation, this is a lossy
    // representation. For example the values `Some(())` and `None` both
    // serialize as just `null`. Unfortunately this is typically what people
    // expect when working with JSON. Other formats are encouraged to behave
    // more intelligently if possible.
    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Option(ref inner) => match inner {
                Some(ref v) => visitor.visit_some(&mut Deserializer::new(v)),
                None => visitor.visit_none(),
            },
            _ => Err(Error::custom("not a union")),
        }
    }

    // In Serde, unit means an anonymous value containing no data.
    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Unit => visitor.visit_unit(),
            _ => Err(Error::custom("not a null")),
        }
    }

    // Unit struct means a named value containing no data.
    fn deserialize_unit_struct<V>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_unit(visitor)
    }

    // As is done here, serializers are encouraged to treat newtype structs as
    // insignificant wrappers around the data they contain. That means not
    // parsing anything other than the contained value.
    fn deserialize_newtype_struct<V>(
        self,
        _: &'static str,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_newtype_struct(self)
    }

    // Deserialization of compound types like sequences and maps happens by
    // passing the visitor an "Access" object that gives it the ability to
    // iterate through the data contained in the sequence.
    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::List(ref items) => visitor.visit_seq(SeqDeserializer::new(items)),
            _ => Err(Error::custom("not an array")),
        }
    }

    // Tuples look just like sequences in JSON. Some formats may be able to
    // represent tuples more efficiently.
    //
    // As indicated by the length parameter, the `Deserialize` implementation
    // for a tuple in the Serde data model is required to know the length of the
    // tuple before even looking at the input data.
    fn deserialize_tuple<V>(self, _: usize, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Tuple structs look just like sequences in JSON.
    fn deserialize_tuple_struct<V>(
        self,
        _: &'static str,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

    // Much like `deserialize_seq` but calls the visitors `visit_map` method
    // with a `MapAccess` implementation, rather than the visitor's `visit_seq`
    // method with a `SeqAccess` implementation.
    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        unimplemented!("Map type is not supported")
    }

    // Structs look just like maps in JSON.
    //
    // Notice the `fields` parameter - a "struct" in the Serde data model means
    // that the `Deserialize` implementation is required to know what the fields
    // are before even looking at the input data. Any key-value pairing in which
    // the fields cannot be known ahead of time is probably a map.
    fn deserialize_struct<V>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        match *self.input {
            Value::Record(ref fields) => visitor.visit_map(StructDeserializer::new(fields)),
            _ => Err(Error::custom(format!(
                "not a record but a {:?}",
                self.input
            ))),
        }
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_enum(EnumDeserializer::new(self))
    }

    // An identifier in Serde is the type that identifies a field of a struct or
    // the variant of an enum. In JSON, struct fields and enum variants are
    // represented as strings. In other formats they may be represented as
    // numeric indices.
    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    // Like `deserialize_any` but indicates to the `Deserializer` that it makes
    // no difference which `Visitor` method is called because the data is
    // ignored.
    //
    // Some deserializers are able to implement this more efficiently than
    // `deserialize_any`, for example by rapidly skipping over matched
    // delimiters without paying close attention to the data in between.
    //
    // Some formats are not able to implement this at all. Formats that can
    // implement `deserialize_any` and `deserialize_ignored_any` are known as
    // self-describing.
    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_unit()
    }
}

impl<'de> de::SeqAccess<'de> for SeqDeserializer<'de> {
    type Error = Error;

    fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.input.next() {
            Some(item) => seed.deserialize(&mut Deserializer::new(&item)).map(Some),
            None => Ok(None),
        }
    }
}

impl<'de> de::MapAccess<'de> for StructDeserializer<'de> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error>
    where
        K: DeserializeSeed<'de>,
    {
        match self.input.next() {
            Some(item) => {
                let (ref field, ref value) = *item;
                self.value = Some(value);
                seed.deserialize(StringDeserializer {
                    input: field.clone(),
                })
                .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        match self.value.take() {
            Some(value) => seed.deserialize(&mut Deserializer::new(value)),
            None => Err(Error::custom("should not happen - too many values")),
        }
    }
}

struct StringDeserializer {
    input: String,
}

impl<'de> de::Deserializer<'de> for StringDeserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        visitor.visit_string(self.input)
    }

    forward_to_deserialize_any! {
        bool u8 u16 u32 u64 i8 i16 i32 i64 f32 f64 char str string unit option
        seq bytes byte_buf map unit_struct newtype_struct
        tuple_struct struct tuple enum identifier ignored_any
    }
}

struct EnumDeserializer<'a, 'de: 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'a, 'de> EnumDeserializer<'a, 'de> {
    fn new(de: &'a mut Deserializer<'de>) -> Self {
        EnumDeserializer { de }
    }
}

impl<'de, 'a> de::EnumAccess<'de> for EnumDeserializer<'a, 'de> {
    type Error = Error;
    type Variant = Self;

    fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
    where
        V: DeserializeSeed<'de>,
    {
        match self.de.input {
            Value::Tag(variant, _) => {
                let val = variant.as_str().into_deserializer();
                seed.deserialize(val).map(|s| (s, self))
            }
            _ => Err(Error::custom(format!(
                "variant_seed: not an enum but a {:?}",
                self.de.input
            ))),
        }
    }
}

impl<'de, 'a> de::VariantAccess<'de> for EnumDeserializer<'a, 'de> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T>(self, seed: T) -> Result<T::Value, Self::Error>
    where
        T: DeserializeSeed<'de>,
    {
        match self.de.input {
            Value::Tag(_, tag_value) => seed.deserialize(&mut Deserializer::new(tag_value)),
            _ => Err(Error::custom(format!(
                "not an enum but a {:?}",
                self.de.input
            ))),
        }
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Error::custom("tuple_variant not supported"))
    }

    fn struct_variant<V>(
        self,
        _fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, Self::Error>
    where
        V: Visitor<'de>,
    {
        Err(Error::custom("struct_variant not supported"))
    }
}

/// Interpret a `Value` as an instance of type `D`.
///
/// This conversion can fail if the structure of the `Value` does not match the
/// structure expected by `D`.
pub fn from_value<'de, D: Deserialize<'de>>(value: &'de Value) -> Result<D, BinaryReaderError> {
    let mut de = Deserializer::new(value);
    Ok(D::deserialize(&mut de)?)
}

/*
 * -----------------------------------------------------------------------------
 *  BigInt deserialization
 * -----------------------------------------------------------------------------
 */
struct BigIntVisitor;

impl<'de> Visitor<'de> for BigIntVisitor {
    type Value = BigInt;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a hex encoded string")
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let bigint: BigInt = num_bigint::BigInt::parse_bytes(value.as_bytes(), 16)
            .unwrap()
            .into();
        Ok(bigint)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let bigint: BigInt = num_bigint::BigInt::parse_bytes(value.as_bytes(), 16)
            .unwrap()
            .into();
        Ok(bigint)
    }
}

impl<'de> Deserialize<'de> for BigInt {
    fn deserialize<D>(deserializer: D) -> Result<BigInt, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_string(BigIntVisitor)
    }
}

#[cfg(test)]
mod tests {
    use serde::{Deserialize, Serialize};

    use super::*;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Payload {
        a: i32,
        b: Option<bool>,
        c: Option<u64>,
        d: f64,
    }

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct Message {
        a: i32,
        p: Payload,
    }

    #[test]
    fn can_deserialize() {
        let serialized = Value::Record(vec![
            ("a".to_string(), Value::Int32(23)),
            (
                "p".to_string(),
                Value::Record(vec![
                    ("a".to_string(), Value::Int32(6)),
                    (
                        "b".to_string(),
                        Value::Option(Some(Box::new(Value::Bool(false)))),
                    ),
                    (
                        "c".to_string(),
                        Value::Option(Some(Box::new(Value::Int64(4_752_163_899)))),
                    ),
                    ("d".to_string(), Value::Float(123.4)),
                ]),
            ),
        ]);
        let deserialized = from_value(&serialized).unwrap();
        let expected = Message {
            a: 23,
            p: Payload {
                a: 6,
                b: Some(false),
                c: Some(4_752_163_899),
                d: 123.4,
            },
        };
        assert_eq!(expected, deserialized);
    }
}
