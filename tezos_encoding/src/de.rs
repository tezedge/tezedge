use std::error::{self, Error as StdError};
use std::fmt;
use std::io;
use std::slice::Iter;
use std::string;
use serde::de::{self, Deserialize, DeserializeSeed, Error as SerdeError, Visitor, IntoDeserializer};
use serde::forward_to_deserialize_any;

use crate::encoding::Encoding;
use crate::types::{BigInt, Value};
use crate::hash::{Hash, HashEncoding, from_prefixed_hash};
use failure::_core::mem::uninitialized;

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn encoding_mismatch(encoding: &Encoding, value: &Value) -> Self {
        Error {
            message: format!("Unsupported encoding {:?} for value: {:?}", encoding, value)
        }
    }
    pub fn unsupported_value<T: fmt::Display>(value: &T) -> Self {
        Error {
            message: format!("Unsupported value: {}", value)
        }
    }
}

impl SerdeError for Error {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Error {
            message: msg.to_string(),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(error::Error::description(self))
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
            message: format!("I/O error. Reason: {:?}", from)
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

impl From<string::FromUtf8Error> for Error {
    fn from(from: string::FromUtf8Error) -> Self {
        Error {
            message: format!("Error decoding UTF-8 string. Reason: {:?}", from),
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
            _ => Err(Error::custom(format!("Unsupported value of type {:?} in deserialize_any.", self.input))),
        }
    }

    forward_to_deserialize_any! {
        bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64
    }

    fn deserialize_char<V>(self, _: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        Err(Error::custom("avro does not support char"))
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_str(s),
            Value::Bytes(ref bytes) => ::std::str::from_utf8(bytes)
                .map_err(|e| Error::custom(e.description()))
                .and_then(|s| visitor.visit_str(s)),
            _ => Err(Error::custom(format!("not a string|bytes|fixed but a {:?}", self.input))),
        }
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_string(s.to_owned()),
            Value::Bytes(ref bytes) => {
                String::from_utf8(bytes.to_owned())
                    .map_err(|e| Error::custom(e.description()))
                    .and_then(|s| visitor.visit_string(s))
            },
            _ => Err(Error::custom(format!("not a string|bytes|fixed but a {:?}", self.input))),
        }
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_bytes(s.as_bytes()),
            Value::Bytes(ref bytes) => visitor.visit_bytes(bytes),
            _ => Err(Error::custom(format!("not a string|bytes|fixed but a {:?}", self.input))),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        match *self.input {
            Value::String(ref s) => visitor.visit_byte_buf(s.clone().into_bytes()),
            Value::Bytes(ref bytes) => {
                visitor.visit_byte_buf(bytes.to_owned())
            },
            _ => Err(Error::custom("not a string|bytes|fixed")),
        }
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        match *self.input {
            Value::Option(ref inner) => {
                match inner {
                    Some(ref v) => visitor.visit_some(&mut Deserializer::new(v)),
                    None => visitor.visit_none()
                }

            },
            _ => Err(Error::custom("not a union")),
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        match *self.input {
            Value::Unit => visitor.visit_unit(),
            _ => Err(Error::custom("not a null")),
        }
    }

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

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        match *self.input {
            Value::List(ref items) => visitor.visit_seq(SeqDeserializer::new(items)),
            _ => Err(Error::custom("not an array")),
        }
    }

    fn deserialize_tuple<V>(self, _: usize, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        self.deserialize_seq(visitor)
    }

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

    fn deserialize_map<V>(self, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        unimplemented!("Map type is not supported")
    }

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
            _ => Err(Error::custom(format!("not a record but a {:?}", self.input))),
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

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
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
                }).map(Some)
            },
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
            },
            _ => Err(Error::custom(format!("not an enum but a {:?}", self.de.input))),
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
            Value::Tag(_, tag_value) => {
                seed.deserialize(&mut Deserializer::new(tag_value))
            }
            _ => Err(Error::custom(format!("not an enum but a {:?}", self.de.input))),
        }
    }

    fn tuple_variant<V>(self, _len: usize, _visitor: V) -> Result<V::Value, Self::Error>
        where
            V: Visitor<'de>,
    {
        Err(Error::custom("tuple_variant not supported"))
    }

    fn struct_variant<V>(self, _fields: &'static [&'static str], _visitor: V) -> Result<V::Value, Self::Error>
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
pub fn from_value<'de, D: Deserialize<'de>>(value: &'de Value) -> Result<D, Error> {
    let mut de = Deserializer::new(value);
    D::deserialize(&mut de)
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
        let bigint: BigInt = num_bigint::BigInt::parse_bytes(value.as_bytes(), 16).unwrap().into();
        Ok(bigint)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
    {
        let bigint: BigInt = num_bigint::BigInt::parse_bytes(value.as_bytes(), 16).unwrap().into();
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


/*
 * -----------------------------------------------------------------------------
 *  Hash<'static> deserialization
 * -----------------------------------------------------------------------------
 */
struct HashIntVisitor(HashEncoding<'static>);

impl<'de> Visitor<'de> for HashIntVisitor {
    type Value = Hash<'static>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an encoded hash string")
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let hash: Hash = from_prefixed_hash(self.0.get_prefix(), value.as_bytes()).unwrap().into();
        Ok(hash)
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        let hash: Hash = from_prefixed_hash(self.0.get_prefix(), value.as_bytes()).unwrap().into();
        Ok(hash)
    }

}

impl<'de> Deserialize<'de> for Hash<'static> {
    fn deserialize<D>(deserializer: D) -> Result<Hash<'static>, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        unimplemented!("TODO")
//        deserializer.deserialize_string(HashIntVisitor)
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
        d: f64
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
            ("p".to_string(), Value::Record(vec![
                ("a".to_string(), Value::Int32(6)),
                ("b".to_string(), Value::Option(Some(Box::new(Value::Bool(false))))),
                ("c".to_string(), Value::Option(Some(Box::new(Value::Int64(4752163899))))),
                ("d".to_string(), Value::Float(123.4))
            ]))
        ]);
        let deserialized = from_value(&serialized).unwrap();
        let expected = Message {
            a: 23,
            p: Payload {
                a: 6,
                b: Some(false),
                c: Some(4752163899),
                d: 123.4
            }
        };
        assert_eq!(expected, deserialized);
    }
}