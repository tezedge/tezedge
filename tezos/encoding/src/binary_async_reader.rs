// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Tezos binary data reader.

use failure::ResultExt;
use std::{
    cmp::min,
    convert::{TryFrom, TryInto},
    io::Cursor,
};

use bit_vec::BitVec;
use serde::de::Error as SerdeError;
use tokio::io::{AsyncRead, AsyncReadExt, Take};

use std::future::Future;
use std::pin::Pin;

use crate::bit_utils::{BitReverse, BitTrim, Bits, ToBytes};
use crate::de;
use crate::encoding::{Encoding, Field, SchemaType};
use crate::types::{self, Value};

use super::binary_reader::{ActualSize, BinaryReaderError, BinaryReaderErrorKind};

pub type Result<'a> =
    Pin<Box<dyn Future<Output = std::result::Result<Value, BinaryReaderError>> + Send + 'a>>;

pub trait BinaryRead: AsyncRead {
    fn remaining(&self) -> std::result::Result<usize, BinaryReaderError>;
}

impl<R: AsyncRead> BinaryRead for Take<R> {
    fn remaining(&self) -> std::result::Result<usize, BinaryReaderError> {
        Ok(self.limit().try_into()?)
    }
}

impl<T: AsRef<[u8]> + Unpin> BinaryRead for Cursor<T> {
    fn remaining(&self) -> std::result::Result<usize, BinaryReaderError> {
        self.get_ref()
            .as_ref()
            .len()
            .checked_sub(usize::try_from(self.position())?)
            .ok_or_else(|| {
                BinaryReaderErrorKind::ArithmeticOverflow {
                    encoding: "remaining()",
                }
                .into()
            })
    }
}

/// Converts Tezos binary form into rust types.
pub struct BinaryAsyncReader;

impl BinaryAsyncReader {
    /// Construct new instance of the [BinaryReader].
    pub fn new() -> Self {
        Self
    }

    /// Convert Tezos binary data into [intermadiate form](Value). Input binary is parsed according to [`encoding`](Encoding).
    ///
    /// # Examples:
    ///
    /// TODO
    pub async fn from_bytes_async<Buf: AsRef<[u8]>>(
        &self,
        buf: Buf,
        encoding: &Encoding,
    ) -> std::result::Result<Value, BinaryReaderError> {
        let buf = buf.as_ref();
        let mut read = Cursor::new(buf).take(buf.len().try_into()?);
        match self.read_message(&mut read, encoding).await {
            Ok(value) => {
                if read.remaining()? == 0 {
                    Ok(value)
                } else {
                    Err(BinaryReaderErrorKind::Overflow {
                        bytes: read.remaining()?,
                    })?
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Convert Tezos binary stream asyncronously into [intermadiate form](Value). Input binary is parsed according to [`encoding`](Encoding).
    ///
    /// Since the [AsyncRead] can provide data of arbitrary length, `encoding` should be wrapped into either [Encoding::Dynamic] or [Encodin::DynamicBounded].
    ///
    /// # Examples:
    /// TODO
    pub async fn read_dynamic_message<'a>(
        &'a self,
        buf: &'a mut (dyn AsyncRead + Unpin + Send),
        encoding: &'a Encoding,
    ) -> std::result::Result<Value, BinaryReaderError> {
        let value = match encoding {
            Encoding::Dynamic(encoding) => {
                let size = buf.read_u32().await?;
                let take: &mut (dyn BinaryRead + Unpin + Send) = &mut buf.take(size.into());
                let value = self.read_message(take, encoding).await?;
                if take.remaining()? != 0 {
                    Err(BinaryReaderErrorKind::Overflow {
                        bytes: take.remaining()?,
                    })?
                } else {
                    value
                }
            }
            Encoding::BoundedDynamic(max, encoding) => {
                let size = buf.read_u32().await?;
                if usize::try_from(size)? > *max {
                    Err(BinaryReaderErrorKind::EncodingBoundaryExceeded {
                        name: "Encoding::BoundedDynamic".to_string(),
                        boundary: *max,
                        actual: ActualSize::Exact(size.try_into()?),
                    })?
                } else {
                    let take: &mut (dyn BinaryRead + Unpin + Send) = &mut buf.take(size.into());
                    let value = self.read_message(take, encoding).await?;
                    if take.remaining()? != 0 {
                        Err(BinaryReaderErrorKind::Overflow {
                            bytes: take.remaining()?,
                        })?
                    } else {
                        value
                    }
                }
            }
            _ => Err(BinaryReaderErrorKind::UnexpectedEncoding(format!(
                "read_message_async expects Dynamic or BoundedDynamic encoding, got {:?}",
                encoding
            )))?,
        };
        Ok(value)
    }

    pub async fn read_message<'a>(
        &'a self,
        buf: &'a mut (dyn BinaryRead + Unpin + Send),
        encoding: &'a Encoding,
    ) -> std::result::Result<Value, BinaryReaderError> {
        let result = match encoding {
            Encoding::Obj(schema) => self.decode_record(buf, schema).await?,
            Encoding::Tup(encodings) => self.decode_tuple(buf, encodings).await?,
            _ => self.decode_value(buf, encoding).await?,
        };

        Ok(result)
    }

    fn decode_record<'a>(
        &'a self,
        buf: &'a mut (dyn BinaryRead + Unpin + Send),
        schema: &'a [Field],
    ) -> Result<'a> {
        Box::pin(async move {
            let mut values = Vec::with_capacity(schema.len());
            for field in schema {
                let name = field.get_name();
                let encoding = field.get_encoding();
                values.push((
                    name.clone(),
                    self.decode_value(buf, encoding)
                        .await
                        .with_context(|e| e.field(field.get_name()))?,
                ))
            }
            Ok(Value::Record(values))
        })
    }

    fn decode_tuple<'a>(
        &'a self,
        buf: &'a mut (dyn BinaryRead + Unpin + Send),
        encodings: &'a [Encoding],
    ) -> Result<'a> {
        Box::pin(async move {
            let mut values = Vec::with_capacity(encodings.len());
            for encoding in encodings {
                values.push(self.decode_value(buf, encoding).await?)
            }
            Ok(Value::Tuple(values))
        })
    }

    fn decode_value<'a>(
        &'a self,
        buf: &'a mut (dyn BinaryRead + Unpin + Send + 'a),
        encoding: &'a Encoding,
    ) -> Result<'a> {
        Box::pin(async move {
            match encoding {
                Encoding::Unit => Ok(Value::Unit),
                Encoding::Int8 => Ok(Value::Int8(buf.read_i8().await?)),
                Encoding::Uint8 => Ok(Value::Uint8(buf.read_u8().await?)),
                Encoding::Int16 => Ok(Value::Int16(buf.read_i16().await?)),
                Encoding::Uint16 => Ok(Value::Uint16(buf.read_u16().await?)),
                Encoding::Int31 => Ok(Value::Int31(buf.read_i32().await?)),
                Encoding::Int32 => Ok(Value::Int32(buf.read_i32().await?)),
                Encoding::Int64 | Encoding::Timestamp => Ok(Value::Int64(buf.read_i64().await?)),
                Encoding::Float => {
                    let mut float = [0u8; 8];
                    buf.read_exact(&mut float).await?;
                    Ok(Value::Float(f64::from_be_bytes(float)))
                }
                Encoding::Bool => {
                    let b = buf.read_u8().await?;
                    match b {
                        types::BYTE_VAL_TRUE => Ok(Value::Bool(true)),
                        types::BYTE_VAL_FALSE => Ok(Value::Bool(false)),
                        _ => Err(de::Error::custom(format!(
                            "Vas expecting 0xFF or 0x00 but instead got {:X}",
                            b
                        )))?,
                    }
                }
                Encoding::String => {
                    let bytes_sz = buf.read_u32().await?;
                    let mut str_buf = vec![0u8; bytes_sz.try_into()?];
                    buf.read_exact(&mut str_buf).await?;
                    Ok(Value::String(String::from_utf8(str_buf)?))
                }
                Encoding::BoundedString(bytes_max) => {
                    let bytes_sz = buf.read_u32().await?;
                    if usize::try_from(bytes_sz)? > *bytes_max {
                        Err(BinaryReaderErrorKind::EncodingBoundaryExceeded {
                            name: "Encoding::BoundedString".to_string(),
                            boundary: *bytes_max,
                            actual: ActualSize::Exact(bytes_sz.try_into()?),
                        })?
                    } else {
                        let mut str_buf = vec![0u8; bytes_sz.try_into()?];
                        buf.read_exact(&mut str_buf).await?;
                        Ok(Value::String(String::from_utf8(str_buf)?))
                    }
                }
                Encoding::Enum => Ok(Value::Enum(None, Some(u32::from(buf.read_u8().await?)))),
                Encoding::Dynamic(dynamic_encoding) => {
                    let bytes_sz = buf.read_u32().await?;
                    if usize::try_from(bytes_sz)? > buf.remaining()? {
                        Err(BinaryReaderErrorKind::Underflow {
                            bytes: usize::try_from(bytes_sz)? - buf.remaining()?,
                        })?
                    } else {
                        let mut take = buf.take(bytes_sz.into());
                        let value = self.decode_value(&mut take, dynamic_encoding).await?;
                        if take.limit() == 0 {
                            Ok(value)
                        } else {
                            Err(BinaryReaderErrorKind::Overflow {
                                bytes: take.remaining()?,
                            })?
                        }
                    }
                }
                Encoding::BoundedDynamic(max, dynamic_encoding) => {
                    let bytes_sz = buf.read_u32().await?;
                    if usize::try_from(bytes_sz)? > *max {
                        Err(BinaryReaderErrorKind::EncodingBoundaryExceeded {
                            name: "Encoding::BoundedDynamic".to_string(),
                            boundary: *max,
                            actual: ActualSize::Exact(bytes_sz.try_into()?),
                        })?
                    } else if usize::try_from(bytes_sz)? > buf.remaining()? {
                        Err(BinaryReaderErrorKind::Underflow {
                            bytes: usize::try_from(bytes_sz)? - buf.remaining()?,
                        })?
                    } else {
                        let mut take = buf.take(bytes_sz.into());
                        let value = self.decode_value(&mut take, dynamic_encoding).await?;
                        if take.limit() == 0 {
                            Ok(value)
                        } else {
                            Err(BinaryReaderErrorKind::Overflow {
                                bytes: take.remaining()?,
                            })?
                        }
                    }
                }
                Encoding::Sized(sized_size, sized_encoding) => {
                    let mut take =
                        buf.take(min((*sized_size).try_into()?, buf.remaining()?.try_into()?));
                    let value = self.decode_value(&mut take, sized_encoding).await?;
                    if take.limit() == 0 {
                        Ok(value)
                    } else {
                        Err(BinaryReaderErrorKind::Underflow {
                            bytes: take.limit().try_into()?,
                        })?
                    }
                }
                Encoding::Bounded(max, inner_encoding) => {
                    let remaining = buf.remaining()?.try_into()?;
                    self.decode_value(
                        &mut buf.take(min((*max).try_into()?, remaining)),
                        inner_encoding,
                    )
                    .await
                    .map_err(|e| match e.kind() {
                        BinaryReaderErrorKind::IOError(_, std::io::ErrorKind::UnexpectedEof) => {
                            BinaryReaderErrorKind::EncodingBoundaryExceeded {
                                name: "Encoding::Bounded".to_string(),
                                boundary: *max,
                                actual: ActualSize::GreaterThan(*max),
                            }
                            .into()
                        }
                        _ => e,
                    })
                }
                Encoding::Greedy(un_sized_encoding) => {
                    self.decode_value(buf, un_sized_encoding).await
                }
                Encoding::Tags(tag_sz, ref tag_map) => {
                    let tag_id = match tag_sz {
                        /*u8*/ 1 => Ok(u16::from(buf.read_u8().await?)),
                        /*u16*/ 2 => Ok(buf.read_u16().await?),
                        _ => Err(de::Error::custom(format!(
                            "Unsupported tag size {}",
                            tag_sz
                        ))),
                    }?;

                    match tag_map.find_by_id(tag_id) {
                        Some(tag) => {
                            let tag_value = self.decode_value(buf, tag.get_encoding()).await?;
                            Ok(Value::Tag(
                                tag.get_variant().to_string(),
                                Box::new(tag_value),
                            ))
                        }
                        None => Err(BinaryReaderErrorKind::UnsupportedTag { tag: tag_id })?,
                    }
                }
                Encoding::List(encoding_inner) => {
                    let mut values = vec![];
                    while buf.remaining()? != 0 {
                        values.push(
                            self.decode_value(buf, encoding_inner)
                                .await
                                .with_context(|e| e.element_of())?,
                        );
                    }
                    Ok(Value::List(values))
                }
                Encoding::BoundedList(max, encoding_inner) => {
                    let mut values = vec![];
                    while buf.remaining()? != 0 {
                        if values.len() >= *max {
                            return Err(BinaryReaderErrorKind::EncodingBoundaryExceeded {
                                name: "Encoding::List".to_string(),
                                boundary: *max,
                                actual: ActualSize::GreaterThan(*max),
                            })?;
                        }
                        values.push(
                            self.decode_value(buf, encoding_inner)
                                .await
                                .with_context(|e| e.element_of())?,
                        );
                    }
                    Ok(Value::List(values))
                }
                Encoding::Option(inner_encoding) => {
                    let is_present_byte = buf.read_u8().await?;
                    match is_present_byte {
                        types::BYTE_VAL_SOME => {
                            let v = self.decode_value(buf, inner_encoding).await?;
                            Ok(Value::Option(Some(Box::new(v))))
                        }
                        types::BYTE_VAL_NONE => Ok(Value::Option(None)),
                        _ => Err(de::Error::custom(format!(
                            "Unexpected option value {:X}",
                            is_present_byte
                        )))?,
                    }
                }
                Encoding::OptionalField(inner_encoding) => {
                    let is_present_byte = buf.read_u8().await?;
                    match is_present_byte {
                        types::BYTE_FIELD_SOME => {
                            let v = self.decode_value(buf, inner_encoding).await?;
                            Ok(Value::Option(Some(Box::new(v))))
                        }
                        types::BYTE_FIELD_NONE => Ok(Value::Option(None)),
                        _ => Err(de::Error::custom(format!(
                            "Unexpected option value {:X}",
                            is_present_byte
                        )))?,
                    }
                }
                Encoding::Obj(schema_inner) => Ok(self.decode_record(buf, schema_inner).await?),
                Encoding::Tup(encodings_inner) => {
                    Ok(self.decode_tuple(buf, encodings_inner).await?)
                }
                Encoding::Z => {
                    // read first byte
                    let byte = buf.read_u8().await?;
                    let negative = byte.get(6)?;
                    if byte <= 0x3F {
                        let mut num = i32::from(byte);
                        if negative {
                            num *= -1;
                        }
                        Ok(Value::String(format!("{:x}", num)))
                    } else {
                        let mut bits = BitVec::new();
                        for bit_idx in 0..6 {
                            bits.push(byte.get(bit_idx)?);
                        }

                        let mut has_next_byte = true;
                        while has_next_byte {
                            let byte = buf.read_u8().await?;
                            for bit_idx in 0..7 {
                                bits.push(byte.get(bit_idx)?)
                            }

                            has_next_byte = byte.get(7)?;
                        }

                        let bytes = bits.reverse().trim_left().to_byte_vec();

                        let mut str_num = bytes
                            .iter()
                            .enumerate()
                            .map(|(idx, b)| match idx {
                                0 => format!("{:x}", *b),
                                _ => format!("{:02x}", *b),
                            })
                            .fold(String::new(), |mut str_num, val| {
                                str_num.push_str(&val);
                                str_num
                            });
                        if negative {
                            str_num = String::from("-") + &str_num;
                        }

                        Ok(Value::String(str_num))
                    }
                }
                Encoding::Mutez => {
                    let mut bits = BitVec::new();

                    let mut has_next_byte = true;
                    while has_next_byte {
                        let byte = buf.read_u8().await?;
                        for bit_idx in 0..7 {
                            bits.push(byte.get(bit_idx)?)
                        }

                        has_next_byte = byte.get(7)?;
                    }

                    let bytes = bits.reverse().trim_left().to_byte_vec();

                    let str_num = bytes
                        .iter()
                        .enumerate()
                        .map(|(idx, b)| match idx {
                            0 => format!("{:x}", *b),
                            _ => format!("{:02x}", *b),
                        })
                        .fold(String::new(), |mut str_num, val| {
                            str_num.push_str(&val);
                            str_num
                        });

                    Ok(Value::String(str_num))
                }
                Encoding::Bytes => {
                    let mut buf_slice = Vec::new();
                    buf.read_to_end(&mut buf_slice).await?;
                    Ok(Value::List(
                        buf_slice.iter().map(|&byte| Value::Uint8(byte)).collect(),
                    ))
                }
                Encoding::Hash(hash_type) => {
                    let bytes_sz = hash_type.size();
                    let mut buf_slice = vec![0u8; bytes_sz];
                    buf.read_exact(&mut buf_slice).await?;
                    Ok(Value::List(
                        buf_slice.iter().map(|&byte| Value::Uint8(byte)).collect(),
                    ))
                }
                Encoding::Split(inner_encoding) => {
                    let inner_encoding = inner_encoding(SchemaType::Binary);
                    self.decode_value(buf, &inner_encoding).await
                }
                Encoding::Lazy(fn_encoding) => {
                    let inner_encoding = fn_encoding();
                    self.decode_value(buf, &inner_encoding).await
                }
                Encoding::Custom(codec) => codec.decode_async(buf, encoding).await,
                Encoding::Uint32 | Encoding::RangedInt | Encoding::RangedFloat => Err(
                    de::Error::custom(format!("Unsupported encoding {:?}", encoding)),
                )?,
            }
        })
    }
}

#[cfg(test)]
mod async_tests {
    use std::mem::size_of;

    use std::io::Cursor;

    use serde::{Deserialize, Serialize};

    use crate::encoding::{Tag, TagMap};
    use crate::ser::Serializer;
    use crate::types::BigInt;
    use crate::{binary_writer, de};

    use super::*;

    #[async_std::test]
    async fn can_deserialize_mutez_from_binary() {
        #[derive(Deserialize, Debug)]
        struct Record {
            a: BigInt,
        }
        let record_schema = vec![Field::new("a", Encoding::Mutez)];

        let mut record_buf = Cursor::new(hex::decode("9e9ed49d01").unwrap());
        let reader = BinaryAsyncReader::new();
        let value = reader
            .read_message(&mut record_buf, &Encoding::Obj(record_schema))
            .await
            .unwrap();
        assert_eq!(
            Value::Record(vec![(
                "a".to_string(),
                Value::String("13b50f1e".to_string())
            )]),
            value
        )
    }

    #[async_std::test]
    async fn can_deserialize_z_from_binary() {
        #[derive(Deserialize, Debug)]
        struct Record {
            a: BigInt,
        }
        let record_schema = vec![Field::new("a", Encoding::Z)];

        let record_buf = hex::decode("9e9ed49d01").unwrap();
        let reader = BinaryAsyncReader::new();
        let value = reader
            .read_message(&mut Cursor::new(record_buf), &Encoding::Obj(record_schema))
            .await
            .unwrap();
        assert_eq!(
            Value::Record(vec![(
                "a".to_string(),
                Value::String("9da879e".to_string())
            )]),
            value
        )
    }

    #[async_std::test]
    async fn can_deserialize_tag_from_binary() {
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
                    Encoding::Obj(get_head_record_schema),
                )]),
            ))),
        )];

        // deserialize to value
        let record_buf = hex::decode("0000000600108eceda2f").unwrap();
        let reader = BinaryAsyncReader::new();
        let value = reader
            .read_message(
                &mut Cursor::new(record_buf),
                &Encoding::Obj(response_schema),
            )
            .await
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

    #[async_std::test]
    async fn can_deserialize_z_range() {
        #[derive(Serialize, Deserialize, Debug)]
        struct Record {
            a: BigInt,
        }
        let record_schema = vec![Field::new("a", Encoding::Z)];
        let record_encoding = Encoding::Obj(record_schema);

        for num in -100..=100 {
            let num_mul = num * 1000;
            let record = Record {
                a: num_bigint::BigInt::from(num_mul).into(),
            };

            let mut serializer = Serializer::default();

            let value_serialized = record.serialize(&mut serializer).unwrap();
            let record_bytes = binary_writer::write(&record, &record_encoding).unwrap();

            let reader = BinaryAsyncReader::new();
            let value_deserialized = reader
                .read_message(&mut Cursor::new(record_bytes), &record_encoding)
                .await
                .unwrap();

            assert_eq!(value_serialized, value_deserialized)
        }
    }

    #[async_std::test]
    async fn can_deserialize_connection_message() {
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
            Field::new("versions", Encoding::list(Encoding::Obj(version_schema))),
        ];
        let connection_message_encoding = Encoding::Obj(connection_message_schema);

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
        let reader = BinaryAsyncReader::new();
        let value = reader
            .read_message(
                &mut Cursor::new(connection_message_buf),
                &connection_message_encoding,
            )
            .await
            .unwrap();

        let connection_message_deserialized: ConnectionMessage = de::from_value(&value).unwrap();
        assert_eq!(connection_message, connection_message_deserialized);
    }

    #[async_std::test]
    async fn can_deserialize_option_some() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Record {
            pub forking_block_hash: Vec<u8>,
        }

        let record_schema = vec![Field::new(
            "forking_block_hash",
            Encoding::list(Encoding::Uint8),
        )];
        let record_encoding = Encoding::Obj(record_schema);

        let record = Some(Record {
            forking_block_hash: hex::decode(
                "2253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b",
            )
            .unwrap(),
        });

        let message_buf =
            hex::decode("012253698f0c94788689fb95ca35eb1535ec3a8b7c613a97e6683f8007d7959e4b")
                .unwrap();
        let reader = BinaryAsyncReader::new();
        let value = reader
            .read_message(
                &mut Cursor::new(message_buf),
                &Encoding::option(record_encoding),
            )
            .await
            .unwrap();

        let record_deserialized: Option<Record> = de::from_value(&value).unwrap();
        assert_eq!(record, record_deserialized);
    }

    #[async_std::test]
    async fn can_deserialize_option_none() {
        #[derive(Deserialize, Debug, PartialEq)]
        struct Record {
            pub forking_block_hash: Vec<u8>,
        }

        let record_schema = vec![Field::new(
            "forking_block_hash",
            Encoding::list(Encoding::Uint8),
        )];
        let record_encoding = Encoding::Obj(record_schema);

        let record: Option<Record> = None;

        let message_buf = hex::decode("00").unwrap();
        let reader = BinaryAsyncReader::new();
        let value = reader
            .read_message(
                &mut Cursor::new(message_buf),
                &Encoding::option(record_encoding),
            )
            .await
            .unwrap();

        let record_deserialized: Option<Record> = de::from_value(&value).unwrap();
        assert_eq!(record, record_deserialized);
    }

    #[async_std::test]
    async fn deserialize_bounds_error_location_string() {
        let schema = Encoding::Obj(vec![Field::new("xxx", Encoding::BoundedString(1))]);
        let data = hex::decode("000000020000").unwrap();
        let err = BinaryAsyncReader::new()
            .read_message(&mut Cursor::new(data), &schema)
            .await
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

    #[async_std::test]
    async fn deserialize_bounds_error_location_list() {
        let schema = Encoding::Obj(vec![Field::new(
            "xxx",
            Encoding::bounded_list(1, Encoding::Uint8),
        )]);
        let data = hex::decode("0000").unwrap();
        let err = BinaryAsyncReader::new()
            .read_message(&mut Cursor::new(data), &schema)
            .await
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

    #[async_std::test]
    async fn deserialize_bounds_error_location_element_of() {
        let schema = Encoding::Obj(vec![Field::new(
            "xxx",
            Encoding::list(Encoding::BoundedString(1)),
        )]);
        let data = hex::decode("000000020000").unwrap();
        let err = BinaryAsyncReader::new()
            .read_message(&mut Cursor::new(data), &schema)
            .await
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

    #[async_std::test]
    async fn underflow_in_bounded() {
        let encoded = hex::decode("000000ff00112233445566778899AABBCCDDEEFF").unwrap(); // dynamic block states 255 bytes, got only 16
        let encoding = Encoding::bounded(1000, Encoding::dynamic(Encoding::list(Encoding::Uint8)));
        let err = BinaryAsyncReader::new()
            .read_message(&mut Cursor::new(encoded), &encoding)
            .await
            .expect_err("Error is expected");
        assert!(
            matches!(err.kind(), BinaryReaderErrorKind::Underflow { .. }),
            "Underflow error expected, got '{:?}'",
            err
        );
    }
}
