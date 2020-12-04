// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Tezos json data writer.

use chrono::{TimeZone, Utc};
use num_traits::Num;
use serde::ser::{Error as SerdeError, Serialize};

use crate::encoding::{Encoding, Field, SchemaType};
use crate::ser::{Error, Serializer};
use crate::types::Value;

pub struct JsonWriter {
    data: String,
}

impl JsonWriter {
    pub fn new() -> JsonWriter {
        JsonWriter {
            data: String::with_capacity(1024),
        }
    }

    pub fn write<T>(&mut self, data: &T, encoding: &Encoding) -> Result<String, Error>
    where
        T: ?Sized + Serialize,
    {
        let mut serializer = Serializer::default();
        let value = data.serialize(&mut serializer)?;

        match encoding {
            Encoding::Obj(schema) => self.encode_record(&value, schema),
            _ => self.encode_value(&value, encoding),
        }?;

        Ok(self.data.clone())
    }

    fn encode_record(&mut self, value: &Value, schema: &[Field]) -> Result<(), Error> {
        match value {
            Value::Record(ref values) => {
                self.open_record();
                for (idx, field) in schema.iter().enumerate() {
                    let name = field.get_name();
                    let value = self
                        .find_value_in_record_values(name, values)
                        .unwrap_or_else(|| panic!("No values found for {}", name));
                    let encoding = field.get_encoding();

                    if idx > 0 {
                        self.push_delimiter();
                    }
                    self.push_key(&name);

                    if let Encoding::Obj(ref schema) = encoding {
                        self.encode_record(value, schema)?
                    } else {
                        self.encode_value(value, encoding)?
                    }
                }
                self.close_record();
                Ok(())
            }
            _ => Err(Error::encoding_mismatch(
                &Encoding::Obj(schema.to_vec()),
                value,
            )),
        }
    }

    fn encode_tuple(&mut self, value: &Value, encodings: &[Encoding]) -> Result<(), Error> {
        Err(Error::unsupported_operation(
            &Encoding::Tup(encodings.to_vec()),
            value,
        ))
    }

    fn push_key(&mut self, key: &str) {
        self.data.push('"');
        self.data.push_str(&key);
        self.data.push_str("\": ");
    }

    fn push_delimiter(&mut self) {
        self.data.push_str(", ");
    }

    fn push_str(&mut self, value: &str) {
        self.data.push('"');
        self.data.push_str(
            &value
                .replace("\"", "\\\"")
                .replace("\n", "\\n")
                .replace("\r", "\\r"),
        );
        self.data.push('"');
    }

    fn push_num<T: Num + ToString>(&mut self, value: T) {
        self.data.push_str(&value.to_string());
    }

    fn push_bool(&mut self, value: bool) {
        if value {
            self.data.push_str("true");
        } else {
            self.data.push_str("false");
        }
    }

    fn push_null(&mut self) {
        self.data.push_str("null")
    }

    fn open_record(&mut self) {
        self.data.push_str("{ ");
    }

    fn close_record(&mut self) {
        self.data.push_str(" }");
    }

    fn open_array(&mut self) {
        self.data.push('[');
    }

    fn close_array(&mut self) {
        self.data.push(']');
    }

    fn encode_value(&mut self, value: &Value, encoding: &Encoding) -> Result<(), Error> {
        match encoding {
            Encoding::Unit => Ok(self.push_null()),
            Encoding::Int8 => match value {
                Value::Int8(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Uint8 => match value {
                Value::Uint8(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Int16 => match value {
                Value::Int16(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Uint16 => match value {
                Value::Uint16(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Int32 => match value {
                Value::Int32(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Int31 => match value {
                Value::Int32(v) => {
                    if (*v & 0x7FFF_FFFF) == *v {
                        Ok(self.push_num(*v))
                    } else {
                        Err(Error::custom("Value is outside of Int31 range"))
                    }
                }
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Uint32 => match value {
                Value::Int32(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::RangedInt => match value {
                Value::RangedInt(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::RangedFloat => match value {
                Value::RangedFloat(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Int64 => match value {
                Value::Int64(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Timestamp => match value {
                Value::Int64(v) => Ok(self.push_str(&Utc.timestamp(*v, 0).to_rfc3339())),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Float => match value {
                Value::Float(v) => Ok(self.push_num(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Bool => match value {
                Value::Bool(v) => Ok(self.push_bool(*v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::String | Encoding::Z | Encoding::Mutez => match value {
                Value::String(v) => Ok(self.push_str(v)),
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Enum => match value {
                Value::Enum(name, _) => {
                    let variant_name = name.as_ref().expect("Was expecting variant name");
                    Ok(self.push_str(variant_name))
                }
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::List(list_inner_encoding) => match value {
                Value::List(ref values) => {
                    self.open_array();
                    for (idx, value) in values.iter().enumerate() {
                        if idx > 0 {
                            self.push_delimiter();
                        }
                        self.encode_value(value, list_inner_encoding)?;
                    }
                    Ok(self.close_array())
                }
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Bytes => match value {
                Value::List(values) => {
                    let mut bytes = vec![];
                    for value in values {
                        match *value {
                                Value::Uint8(u8_val) => {
                                    bytes.push(u8_val);
                                }
                                _ => return Err(Error::custom(format!("Encoding::Bytes could be applied only to &[u8] value but found: {:?}", value)))
                            }
                    }
                    Ok(self.push_str(&hex::encode(bytes)))
                }
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Hash(hash_encoding) => match value {
                Value::List(values) => {
                    let mut bytes = vec![];
                    for value in values {
                        match *value {
                                Value::Uint8(u8_val) => {
                                    bytes.push(u8_val);
                                }
                                _ => return Err(Error::custom(format!("Encoding::Hash could be applied only to &[u8] value but found: {:?}", value)))
                            }
                    }
                    Ok(self.push_str(&hash_encoding.hash_to_b58check(&bytes)))
                }
                _ => Err(Error::encoding_mismatch(encoding, value)),
            },
            Encoding::Option(option_encoding) | Encoding::OptionalField(option_encoding) => {
                match value {
                    Value::Option(wrapped_value) => match wrapped_value {
                        Some(option_value) => self.encode_value(option_value, option_encoding),
                        None => Ok(self.push_null()),
                    },
                    _ => Err(Error::encoding_mismatch(encoding, value)),
                }
            }
            Encoding::Obj(obj_schema) => self.encode_record(value, obj_schema),
            Encoding::Tup(tup_encodings) => self.encode_tuple(value, tup_encodings),
            Encoding::Dynamic(dynamic_encoding) => self.encode_value(value, dynamic_encoding),
            Encoding::Sized(_, sized_encoding) => self.encode_value(value, sized_encoding),
            Encoding::Greedy(un_sized_encoding) => self.encode_value(value, un_sized_encoding),
            Encoding::Tags(_, _) => {
                unimplemented!("Encoding::Tags encoding is not supported for JSON format")
            }
            Encoding::Split(fn_encoding) => {
                let inner_encoding = fn_encoding(SchemaType::Json);
                self.encode_value(value, &inner_encoding)
            }
            Encoding::Lazy(fn_encoding) => {
                let inner_encoding = fn_encoding();
                self.encode_value(value, &inner_encoding)
            }
        }
    }

    fn find_value_in_record_values<'a>(
        &self,
        name: &'a str,
        values: &'a [(String, Value)],
    ) -> Option<&'a Value> {
        values
            .iter()
            .find(|&(v_name, _)| v_name == name)
            .map(|(_, value)| value)
    }
}

#[cfg(test)]
mod tests {
    use serde::Serialize;

    use crypto::hash::HashType;

    use crate::types::BigInt;

    use super::*;

    #[test]
    fn can_serialize_complex_schema_to_json() {
        #[derive(Serialize, Debug)]
        #[allow(dead_code)]
        enum EnumType {
            Accepted,
            Running,
            Disconnected,
        }

        #[derive(Serialize, Debug)]
        struct Version {
            name: String,
            major: u16,
            minor: u16,
        }

        #[derive(Serialize, Debug)]
        struct SubRecord {
            x: i32,
            y: i32,
            v: Vec<i32>,
        }

        #[derive(Serialize, Debug)]
        struct Record {
            a: i32,
            b: bool,
            c: Option<BigInt>,
            d: f64,
            e: EnumType,
            f: Vec<Version>,
            s: SubRecord,
            h: Vec<u8>,
            p: Vec<u8>,
            t: i64,
            ofs: Option<String>,
        }

        let record = Record {
            a: 32,
            b: true,
            c: Some(num_bigint::BigInt::from(1_548_569_249).into()),
            d: 12.34,
            e: EnumType::Disconnected,
            f: vec![
                Version {
                    name: "A".to_string(),
                    major: 1,
                    minor: 1,
                },
                Version {
                    name: "B".to_string(),
                    major: 2,
                    minor: 0,
                },
            ],
            s: SubRecord {
                x: 5,
                y: 32,
                v: vec![12, 34],
            },
            h: hex::decode("8eceda2f").unwrap(),
            p: hex::decode("6cf20139cedef0ed52395a327ad13390d9e8c1e999339a24f8513fe513ed689a")
                .unwrap(),
            t: 1_553_127_011,
            ofs: Some("ofs".to_string()),
        };

        let version_schema = vec![
            Field::new("name", Encoding::String),
            Field::new("major", Encoding::Uint16),
            Field::new("minor", Encoding::Uint16),
        ];

        let sub_record_schema = vec![
            Field::new("x", Encoding::Int31),
            Field::new("y", Encoding::Int31),
            Field::new("v", Encoding::dynamic(Encoding::list(Encoding::Int31))),
        ];

        let record_schema = vec![
            Field::new("a", Encoding::Int31),
            Field::new("b", Encoding::Bool),
            Field::new("t", Encoding::Timestamp),
            Field::new("s", Encoding::Obj(sub_record_schema)),
            Field::new("p", Encoding::sized(32, Encoding::Bytes)),
            Field::new("c", Encoding::Option(Box::new(Encoding::Z))),
            Field::new("d", Encoding::Float),
            Field::new("e", Encoding::Enum),
            Field::new(
                "f",
                Encoding::dynamic(Encoding::list(Encoding::Obj(version_schema))),
            ),
            Field::new("h", Encoding::Hash(HashType::ChainId)),
            Field::new("ofs", Encoding::OptionalField(Box::new(Encoding::String))),
        ];

        let mut writer = JsonWriter::new();
        let writer_result = writer.write(&record, &Encoding::Obj(record_schema));
        assert!(writer_result.is_ok());

        let expected_writer_result = r#"{ "a": 32, "b": true, "t": "2019-03-21T00:10:11+00:00", "s": { "x": 5, "y": 32, "v": [12, 34] }, "p": "6cf20139cedef0ed52395a327ad13390d9e8c1e999339a24f8513fe513ed689a", "c": "5c4d4aa1", "d": 12.34, "e": "Disconnected", "f": [{ "name": "A", "major": 1, "minor": 1 }, { "name": "B", "major": 2, "minor": 0 }], "h": "NetXgtSLGNJvNye", "ofs": "ofs" }"#;
        assert_eq!(expected_writer_result, writer_result.unwrap());
    }
}
