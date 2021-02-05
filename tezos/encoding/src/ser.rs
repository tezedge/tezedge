// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Serde Serializer
//! Inspired by https://serde.rs/impl-serializer.html

use std::error;
use std::fmt;
use std::io;

use serde::ser::{self, Error as _, Serialize};

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

    pub fn unsupported_operation(encoding: &Encoding, value: &Value) -> Self {
        Error {
            message: format!(
                "Unsupported encoding operation {:?} for value: {:?}",
                encoding, value
            ),
        }
    }

    pub fn unimplemented(method: &str) -> Self {
        Error {
            message: format!("Serialization method {} unimplemented", method),
        }
    }
}

impl ser::Error for Error {
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

impl error::Error for Error {
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

impl From<crate::bit_utils::BitsError> for Error {
    fn from(source: crate::bit_utils::BitsError) -> Self {
        Error {
            message: format!("Error operating with bits: {:?}", source),
        }
    }
}

#[derive(Clone, Default)]
pub struct Serializer {}

pub struct SeqSerializer {
    items: Vec<Value>,
    base_serializer: Serializer,
}

pub struct MapSerializer {}

pub struct StructSerializer {
    fields: Vec<(String, Value)>,
    base_serializer: Serializer,
}

impl SeqSerializer {
    pub fn new(len: Option<usize>) -> SeqSerializer {
        let items = match len {
            Some(len) => Vec::with_capacity(len),
            None => Vec::new(),
        };

        SeqSerializer {
            items,
            base_serializer: Serializer::default(),
        }
    }
}

impl StructSerializer {
    pub fn new(len: usize) -> StructSerializer {
        StructSerializer {
            fields: Vec::with_capacity(len),
            base_serializer: Serializer::default(),
        }
    }
}

impl<'b> ser::Serializer for &'b mut Serializer {
    type Ok = Value;
    type Error = Error;
    type SerializeSeq = SeqSerializer;
    type SerializeTuple = SeqSerializer;
    type SerializeTupleStruct = SeqSerializer;
    type SerializeTupleVariant = SeqSerializer;
    type SerializeMap = MapSerializer;
    type SerializeStruct = StructSerializer;
    type SerializeStructVariant = StructSerializer;

    fn serialize_bool(self, v: bool) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bool(v))
    }

    fn serialize_i8(self, v: i8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int8(v))
    }

    fn serialize_i16(self, v: i16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int16(v))
    }

    fn serialize_i32(self, v: i32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int32(v))
    }

    fn serialize_i64(self, v: i64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Int64(v))
    }

    fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Uint8(v))
    }

    fn serialize_u16(self, v: u16) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Uint16(v))
    }

    fn serialize_u32(self, v: u32) -> Result<Self::Ok, Self::Error> {
        if v <= i32::max_value() as u32 {
            Ok(Value::Int32(v as i32))
        } else {
            Ok(Value::Int64(i64::from(v)))
        }
    }

    fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
        if v <= i64::max_value() as u64 {
            Ok(Value::Int64(v as i64))
        } else {
            Err(Error::custom("u64 is too large"))
        }
    }

    fn serialize_f32(self, v: f32) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(f64::from(v)))
    }

    fn serialize_f64(self, v: f64) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Float(v))
    }

    fn serialize_char(self, v: char) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Uint8(v as u8))
    }

    fn serialize_str(self, v: &str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::String(v.to_owned()))
    }

    fn serialize_bytes(self, v: &[u8]) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Bytes(v.to_owned()))
    }

    fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Option(None))
    }

    fn serialize_some<T: ?Sized>(self, value: &T) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        let v = value.serialize(self)?;
        Ok(Value::Option(Some(Box::new(v))))
    }

    fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Unit)
    }

    fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Unit)
    }

    fn serialize_unit_variant(
        self,
        _name: &'static str,
        variant_index: u32,
        variant: &'static str,
    ) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Enum(
            Some(String::from(variant)),
            Some(variant_index),
        ))
    }

    fn serialize_newtype_struct<T: ?Sized>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<Self::Ok, Self::Error>
    where
        T: Serialize,
    {
        let s = value.serialize(self)?;
        Ok(Value::Tag(String::from(variant), Box::new(s)))
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
        Ok(SeqSerializer::new(len))
    }

    fn serialize_tuple(self, len: usize) -> Result<Self::SerializeTuple, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeTupleStruct, Self::Error> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeTupleVariant, Self::Error> {
        Err(Error::unimplemented("Serializer::serialize_tuple_variant"))
    }

    fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
        Err(Error::unimplemented("Serializer::serialize_map"))
    }

    fn serialize_struct(
        self,
        _: &'static str,
        len: usize,
    ) -> Result<Self::SerializeStruct, Self::Error> {
        Ok(StructSerializer::new(len))
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self::SerializeStructVariant, Self::Error> {
        Err(Error::unimplemented("Serializer::serialize_struct_variant"))
    }
}

/*
 * -----------------------------------------------------------------------------
 *  Serializer types
 * -----------------------------------------------------------------------------
 */
impl<'a> ser::SerializeSeq for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.items.push(value.serialize(&mut self.base_serializer)?);
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::List(self.items))
    }
}

impl<'a> ser::SerializeTuple for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_element<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleStruct for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        ser::SerializeSeq::serialize_element(self, value)
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        ser::SerializeSeq::end(self)
    }
}

impl ser::SerializeTupleVariant for SeqSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, _: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        Err(Error::unimplemented(
            "SerializeTupleVariant::serialize_field",
        ))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unimplemented("SerializeTupleVariant::end"))
    }
}

impl ser::SerializeMap for MapSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_key<T: ?Sized>(&mut self, _key: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        Err(Error::unimplemented("SerializeMap::serialize_key"))
    }

    fn serialize_value<T: ?Sized>(&mut self, _value: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        Err(Error::unimplemented("SerializeMap::serialize_value"))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unimplemented("SerializeMap::end"))
    }
}

impl ser::SerializeStruct for StructSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(
        &mut self,
        name: &'static str,
        value: &T,
    ) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        self.fields
            .push((name.to_owned(), value.serialize(&mut self.base_serializer)?));
        Ok(())
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Ok(Value::Record(self.fields))
    }
}

impl ser::SerializeStructVariant for StructSerializer {
    type Ok = Value;
    type Error = Error;

    fn serialize_field<T: ?Sized>(&mut self, _: &'static str, _: &T) -> Result<(), Self::Error>
    where
        T: Serialize,
    {
        Err(Error::unimplemented(
            "SerializeStructVariant::serialize_field",
        ))
    }

    fn end(self) -> Result<Self::Ok, Self::Error> {
        Err(Error::unimplemented("SerializeStructVariant::end"))
    }
}

/*
 * -----------------------------------------------------------------------------
 *  BigInt serialization
 * -----------------------------------------------------------------------------
 */
impl Serialize for BigInt {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let bigint: num_bigint::BigInt = self.into();
        serializer.serialize_str(&bigint.to_str_radix(16))
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use super::*;

    #[derive(Serialize, Debug)]
    struct Payload {
        a: i32,
        b: Option<bool>,
        c: Option<u64>,
        d: f64,
    }

    #[derive(Serialize, Debug)]
    struct Message {
        a: i32,
        p: Payload,
    }

    #[test]
    fn can_serialize() {
        let message = Message {
            a: 23,
            p: Payload {
                a: 6,
                b: Some(false),
                c: Some(4_752_163_899),
                d: 123.4,
            },
        };

        let mut serializer = Serializer::default();
        match message.serialize(&mut serializer) {
            Ok(Value::Record(fields)) => {
                assert_eq!(fields.len(), 2);
                let fld_1 = &fields[0];
                assert_eq!("a", fld_1.0);
                match fld_1.1 {
                    Value::Int32(v) => assert_eq!(23, v),
                    _ => panic!("Was expecting Value::Int32(v)"),
                }
                let fld_2 = &fields[1];
                assert_eq!("p", fld_2.0);
                match fld_2.1 {
                    Value::Record(ref v) => assert_eq!(4, v.len()),
                    _ => panic!("Was expecting &Value::Record(v)"),
                }
            }
            _ => panic!("Invalid  type returned from serialization. Was expecting Value::Record"),
        }
    }
}
