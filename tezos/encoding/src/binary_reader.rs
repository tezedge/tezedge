// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Tezos binary data reader.

use std::{
    fmt::{self, Display},
    str::Utf8Error,
};

use failure::Fail;
use serde::de::Error as SerdeError;

use crate::de;

use super::error_context::EncodingError;

/// Error produced by a [BinaryReader].
pub type BinaryReaderError = EncodingError<BinaryReaderErrorKind>;

/// Actual size of encoded data
#[derive(Debug, Clone, Copy)]
pub enum ActualSize {
    /// Exact size
    Exact(usize),
    /// Unknown size exceeding the limit
    GreaterThan(usize),
}

impl Display for ActualSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ActualSize::Exact(size) => write!(f, "{}", size),
            ActualSize::GreaterThan(size) => write!(f, "greater than {}", size),
        }
    }
}

/// Kind of error for [BinaryReaderError]
#[derive(Debug, Fail, Clone)]
pub enum BinaryReaderErrorKind {
    /// More bytes were expected than there were available in input buffer.
    #[fail(display = "Input underflow, missing {} bytes", bytes)]
    Underflow { bytes: usize },
    /// Writer tried to write too many bytes to output buffer.
    #[fail(display = "Input overflow, excess of {} bytes", bytes)]
    Overflow { bytes: usize },
    /// Generic deserialization error.
    #[fail(display = "Deserializer error: {}", error)]
    DeserializationError { error: crate::de::Error },
    /// No tag with the corresponding id was found. This might not be an error of the binary data but
    /// may simply mean that we have not yet defined tag in encoding.
    #[fail(display = "No tag found for id: 0x{:X}", tag)]
    UnsupportedTag { tag: u16 },
    /// Encoding boundary constraint violation
    #[fail(
        display = "Encoded data {} exceeded its size boundary: {}, actual: {}",
        name, boundary, actual
    )]
    EncodingBoundaryExceeded {
        name: String,
        boundary: usize,
        actual: ActualSize,
    },
    /// Arithmetic overflow
    #[fail(display = "Arithmetic overflow while encoding {:?}", encoding)]
    ArithmeticOverflow { encoding: &'static str },
    #[fail(display = "Nom error: {}", error)]
    NomError { error: String },
    #[fail(display = "Utf8 error: {}", error)]
    Utf8Error { error: Utf8Error },
}

impl From<crate::de::Error> for BinaryReaderError {
    fn from(error: crate::de::Error) -> Self {
        BinaryReaderErrorKind::DeserializationError { error }.into()
    }
}

impl From<std::string::FromUtf8Error> for BinaryReaderError {
    fn from(from: std::string::FromUtf8Error) -> Self {
        BinaryReaderErrorKind::DeserializationError {
            error: de::Error::custom(format!("Error decoding UTF-8 string. Reason: {:?}", from)),
        }
        .into()
    }
}

impl From<crate::bit_utils::BitsError> for BinaryReaderError {
    fn from(source: crate::bit_utils::BitsError) -> Self {
        BinaryReaderErrorKind::DeserializationError {
            error: crate::de::Error::custom(format!("Bits operation error: {:?}", source)),
        }
        .into()
    }
}

impl<'a> From<nom::Err<crate::nom::NomError<'a>>> for BinaryReaderError {
    fn from(error: nom::Err<crate::nom::NomError<'a>>) -> Self {
        BinaryReaderErrorKind::NomError {
            error: error.to_string(),
        }
        .into()
    }
}

impl From<Utf8Error> for BinaryReaderError {
    fn from(error: Utf8Error) -> Self {
        BinaryReaderErrorKind::Utf8Error { error }.into()
    }
}

/*
#[cfg(test)]
mod tests {
    use std::mem::size_of;

    use serde::{Deserialize, Serialize};

    use crate::encoding::{Tag, TagMap};
    use crate::ser::Serializer;
    use crate::types::BigInt;
    use crate::{binary_writer, de};

    use super::*;

    #[test]
    fn can_deserialize_mutez_from_binary() {
        #[derive(Deserialize, Debug)]
        struct Record {
            a: BigInt,
        }
        let record_schema = vec![Field::new("a", Encoding::Mutez)];

        let record_buf = hex::decode("9e9ed49d01").unwrap();
        let reader = BinaryReader::new();
        let value = reader
            .read(record_buf, &Encoding::Obj("", record_schema))
            .unwrap();
        assert_eq!(
            Value::Record(vec![(
                "a".to_string(),
                Value::String("13b50f1e".to_string())
            )]),
            value
        )
    }

    #[test]
    fn can_deserialize_z_from_binary() {
        #[derive(Deserialize, Debug)]
        struct Record {
            a: BigInt,
        }
        let record_schema = vec![Field::new("a", Encoding::Z)];

        let record_buf = hex::decode("9e9ed49d01").unwrap();
        let reader = BinaryReader::new();
        let value = reader
            .read(record_buf, &Encoding::Obj("", record_schema))
            .unwrap();
        assert_eq!(
            Value::Record(vec![(
                "a".to_string(),
                Value::String("9da879e".to_string())
            )]),
            value
        )
    }

    #[test]
    #[ignore = "Error in decoding small negative integers"]
    fn can_deserialize_z_from_binary_negative() {
        let record_schema = &Encoding::Obj("", vec![Field::new("a", Encoding::Z)]);

        let record_buf = hex::decode("53").unwrap();
        let reader = BinaryReader::new();
        let value = reader.read(record_buf, &record_schema).unwrap();
        assert_eq!(
            Value::Record(vec![("a".to_string(), Value::String("-23".to_string()))]),
            value
        )
    }

    #[test]
    fn can_deserialize_tag_from_binary() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct GetHeadRecord {
            chain_id: Vec<u8>,
        }

        let get_head_record_schema = vec![Field::new(
            "chain_id",
            Encoding::Sized(4, Box::new(Encoding::Bytes)),
        )];

        #[derive(Deserialize, Debug, PartialEq)]
        enum Message {
            GetHead(GetHeadRecord),
        }

        #[derive(Deserialize, Debug, PartialEq)]
        struct Response {
            messages: Vec<Message>,
        }

        let response_schema = vec![Field::new(
            "messages",
            Encoding::dynamic(Encoding::list(Encoding::Tags(
                size_of::<u16>(),
                TagMap::new(vec![Tag::new(
                    0x10,
                    "GetHead",
                    Encoding::Obj("GetHead", get_head_record_schema),
                )]),
            ))),
        )];

        // deserialize to value
        let record_buf = hex::decode("0000000600108eceda2f").unwrap();
        let reader = BinaryReader::new();
        let value = reader
            .read(record_buf, &Encoding::Obj("", response_schema))
            .unwrap();
        // convert value to actual data structure
        let value: Response = de::from_value(&value).unwrap();
        let expected_value = Response {
            messages: vec![Message::GetHead(GetHeadRecord {
                chain_id: hex::decode("8eceda2f").unwrap(),
            })],
        };
        assert_eq!(expected_value, value)
    }

    #[test]
    fn can_deserialize_z_range() {
        #[derive(Serialize, Deserialize, Debug)]
        struct Record {
            a: BigInt,
        }
        let record_schema = vec![Field::new("a", Encoding::Z)];
        let record_encoding = Encoding::Obj("", record_schema);

        for num in -100..=100 {
            let num_mul = num * 1000;
            let record = Record {
                a: num_bigint::BigInt::from(num_mul).into(),
            };

            let mut serializer = Serializer::default();

            let value_serialized = record.serialize(&mut serializer).unwrap();
            let record_bytes = binary_writer::write(&record, &record_encoding).unwrap();

            let reader = BinaryReader::new();
            let value_deserialized = reader.read(record_bytes, &record_encoding).unwrap();

            assert_eq!(value_serialized, value_deserialized)
        }
    }

    #[test]
    fn can_deserialize_connection_message() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Version {
            name: String,
            major: u16,
            minor: u16,
        }

        #[derive(Deserialize, Debug, PartialEq)]
        struct ConnectionMessage {
            port: u16,
            versions: Vec<Version>,
            public_key: Vec<u8>,
            proof_of_work_stamp: Vec<u8>,
            message_nonce: Vec<u8>,
        }

        let version_schema = vec![
            Field::new("name", Encoding::String),
            Field::new("major", Encoding::Uint16),
            Field::new("minor", Encoding::Uint16),
        ];

        let connection_message_schema = vec![
            Field::new("port", Encoding::Uint16),
            Field::new("public_key", Encoding::sized(32, Encoding::Bytes)),
            Field::new("proof_of_work_stamp", Encoding::sized(24, Encoding::Bytes)),
            Field::new("message_nonce", Encoding::sized(24, Encoding::Bytes)),
            Field::new(
                "versions",
                Encoding::list(Encoding::Obj("", version_schema)),
            ),
        ];
        let connection_message_encoding = Encoding::Obj("", connection_message_schema);

        let connection_message = ConnectionMessage {
            port: 3001,
            versions: vec![
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
            public_key: hex::decode(
                "eaef40186db19fd6f56ed5b1af57f9d9c8a1eed85c29f8e4daaa7367869c0f0b",
            )
            .unwrap(),
            proof_of_work_stamp: hex::decode("000000000000000000000000000000000000000000000000")
                .unwrap(),
            message_nonce: hex::decode("000000000000000000000000000000000000000000000000").unwrap(),
        };

        let connection_message_buf = hex::decode("0bb9eaef40186db19fd6f56ed5b1af57f9d9c8a1eed85c29f8e4daaa7367869c0f0b000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014100010001000000014200020000").unwrap();
        let reader = BinaryReader::new();
        let value = reader
            .read(connection_message_buf, &connection_message_encoding)
            .unwrap();

        let connection_message_deserialized: ConnectionMessage = de::from_value(&value).unwrap();
        assert_eq!(connection_message, connection_message_deserialized);
    }

    #[test]
    fn can_deserialize_option_some() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Record {
            pub forking_block_hash: Vec<u8>,
        }

        let record_schema = vec![Field::new(
            "forking_block_hash",
            Encoding::list(Encoding::Uint8),
        )];
        let record_encoding = Encoding::Obj("", record_schema);

        let record = Some(Record {
            forking_block_hash: hex::decode(
                "2253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b",
            )
            .unwrap(),
        });

        let message_buf =
            hex::decode("012253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b")
                .unwrap();
        let reader = BinaryReader::new();
        let value = reader
            .read(message_buf, &Encoding::option(record_encoding))
            .unwrap();

        let record_deserialized: Option<Record> = de::from_value(&value).unwrap();
        assert_eq!(record, record_deserialized);
    }

    #[test]
    fn can_deserialize_option_none() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Record {
            pub forking_block_hash: Vec<u8>,
        }

        let record_schema = vec![Field::new(
            "forking_block_hash",
            Encoding::list(Encoding::Uint8),
        )];
        let record_encoding = Encoding::Obj("", record_schema);

        let record: Option<Record> = None;

        let message_buf = hex::decode("00").unwrap();
        let reader = BinaryReader::new();
        let value = reader
            .read(message_buf, &Encoding::option(record_encoding))
            .unwrap();

        let record_deserialized: Option<Record> = de::from_value(&value).unwrap();
        assert_eq!(record, record_deserialized);
    }

    #[test]
    fn deserialize_bounds_error_location_string() {
        let schema = Encoding::Obj("", vec![Field::new("xxx", Encoding::BoundedString(1))]);
        let data = hex::decode("000000020000").unwrap();
        let err = BinaryReader::new()
            .read(data, &schema)
            .expect_err("Error is expected");
        let kind = err.kind();
        assert!(matches!(
            kind,
            BinaryReaderErrorKind::EncodingBoundaryExceeded {
                name: _,
                boundary: 1,
                actual: ActualSize::Exact(2)
            }
        ));
        let location = err.location();
        assert!(location.contains("field `xxx`"));
    }

    #[test]
    fn deserialize_bounds_error_location_list() {
        let schema = Encoding::Obj(
            "",
            vec![Field::new(
                "xxx",
                Encoding::bounded_list(1, Encoding::Uint8),
            )],
        );
        let data = hex::decode("0000").unwrap();
        let err = BinaryReader::new()
            .read(data, &schema)
            .expect_err("Error is expected");
        let kind = err.kind();
        assert!(matches!(
            kind,
            BinaryReaderErrorKind::EncodingBoundaryExceeded {
                name: _,
                boundary: 1,
                actual: ActualSize::GreaterThan(1)
            }
        ));
        let location = err.location();
        assert!(location.contains("field `xxx`"));
    }

    #[test]
    fn deserialize_bounds_error_location_element_of() {
        let schema = Encoding::Obj(
            "",
            vec![Field::new(
                "xxx",
                Encoding::list(Encoding::BoundedString(1)),
            )],
        );
        let data = hex::decode("000000020000").unwrap();
        let err = BinaryReader::new()
            .read(data, &schema)
            .expect_err("Error is expected");
        let kind = err.kind();
        assert!(matches!(
            kind,
            BinaryReaderErrorKind::EncodingBoundaryExceeded {
                name: _,
                boundary: 1,
                actual: ActualSize::Exact(2)
            }
        ));
        let location = err.location();
        assert!(location.contains("field `xxx`"));
        assert!(location.contains("list element"));
    }

    #[test]
    fn underflow_in_bounded() {
        let encoded = hex::decode("000000ff00112233445566778899AABBCCDDEEFF").unwrap(); // dynamic block states 255 bytes, got only 16
        let encoding = Encoding::bounded(1000, Encoding::dynamic(Encoding::list(Encoding::Uint8)));
        let err = BinaryReader::new()
            .read(encoded, &encoding)
            .expect_err("Error is expected");
        assert!(
            matches!(err.kind(), BinaryReaderErrorKind::Underflow { .. }),
            "Underflow error expected, got '{:?}'",
            err
        );
    }
}
*/
