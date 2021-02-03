// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::sync::Arc;

use bytes::Buf;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize, Serializer};

use crypto::hash::{BlockHash, Hash, HashType};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::encoding::{CustomCodec, Encoding, Field, HasEncoding};
use tezos_encoding::ser::Error;
use tezos_encoding::types::{RecursiveDataSize, Value};
use tezos_encoding::{has_encoding, safe};

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;
use crate::p2p::encoding::operation::Operation;
use tezos_encoding::json_writer::JsonWriter;

/// Maximal depth for path to be decoded/encoded.
/// Serde serialization is still recursive and can lead to stack overflow.
/// This value can be handled by serde and still it is big enough.
/// to handle 2 ** 512 elements tree.
pub const PATH_MAX_DEPTH: RecursiveDataSize = 128;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, CopyGetters, Getters)]
pub struct OperationsForBlock {
    #[get = "pub"]
    hash: BlockHash,
    #[get_copy = "pub"]
    validation_pass: i8,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl OperationsForBlock {
    pub fn new(hash: BlockHash, validation_pass: i8) -> Self {
        OperationsForBlock {
            hash,
            validation_pass,
            body: Default::default(),
        }
    }

    /// alternative getter because .hash() causes problem with hash() method from Hash trait
    #[inline(always)]
    pub fn block_hash(&self) -> &BlockHash {
        &self.hash
    }
}

cached_data!(OperationsForBlock, body);
has_encoding!(OperationsForBlock, OPERATIONS_FOR_BLOCK_ENCODING, {
    Encoding::Obj(vec![
        Field::new("hash", Encoding::Hash(HashType::BlockHash)),
        Field::new("validation_pass", Encoding::Int8),
    ])
});
// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct OperationsForBlocksMessage {
    #[get = "pub"]
    operations_for_block: OperationsForBlock,
    #[get = "pub"]
    operation_hashes_path: Path,
    #[get = "pub"]
    operations: Vec<Operation>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl OperationsForBlocksMessage {
    pub fn new(
        operations_for_block: OperationsForBlock,
        operation_hashes_path: Path,
        operations: Vec<Operation>,
    ) -> Self {
        OperationsForBlocksMessage {
            operations_for_block,
            operation_hashes_path,
            operations,
            body: Default::default(),
        }
    }
}

cached_data!(OperationsForBlocksMessage, body);
has_encoding!(
    OperationsForBlocksMessage,
    OPERATIONS_FOR_BLOCKS_MESSAGE_ENCODING,
    {
        Encoding::Obj(vec![
            Field::new(
                "operations_for_block",
                OperationsForBlock::encoding().clone(),
            ),
            Field::new("operation_hashes_path", PathCodec::get_encoding()),
            Field::new(
                "operations",
                Encoding::list(Encoding::dynamic(Operation::encoding().clone())),
            ),
        ])
    }
);

impl From<OperationsForBlocksMessage> for Vec<Operation> {
    fn from(msg: OperationsForBlocksMessage) -> Self {
        msg.operations
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, PartialEq, Debug, Getters)]
pub struct PathRight {
    #[get = "pub"]
    left: Hash,
    #[get = "pub"]
    path: Path,
    /// Depth (lenght) of the path
    #[serde(skip_serializing)]
    depth: Option<RecursiveDataSize>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(PathRight, body);
has_encoding!(PathRight, PATH_RIGHT_ENCODING, {
    Encoding::Obj(vec![
        Field::new("left", Encoding::Hash(HashType::OperationListListHash)),
        Field::new("path", PathCodec::get_encoding()),
    ])
});

impl PathRight {
    pub fn new(left: Hash, path: Path, body: BinaryDataCache) -> Self {
        let depth = path.depth().map(|d| d.checked_add(1)).flatten();
        Self {
            left,
            path,
            depth,
            body,
        }
    }
}

/// Custom deserialization is needed to properly set `depth` field.
/// See https://serde.rs/deserialize-struct.html
impl<'de> Deserialize<'de> for PathRight {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        use serde::de::{self, Error, SeqAccess, Visitor};
        use std::fmt;
        enum Field {
            Left,
            Path,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: de::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`left` or `path`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "left" => Ok(Field::Left),
                            "path" => Ok(Field::Path),
                            _ => Err(Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PathRightVisitor;

        impl<'de> Visitor<'de> for PathRightVisitor {
            type Value = PathRight;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct PathRight")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<PathRight, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let path = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let left = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(PathRight::new(path, left, Default::default()))
            }

            fn visit_map<V>(self, mut map: V) -> Result<PathRight, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut path = None;
                let mut left = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Left => {
                            if left.is_some() {
                                return Err(de::Error::duplicate_field("left"));
                            }
                            left = Some(map.next_value()?);
                        }
                        Field::Path => {
                            if path.is_some() {
                                return Err(de::Error::duplicate_field("path"));
                            }
                            path = Some(map.next_value()?);
                        }
                    }
                }
                let left = left.ok_or_else(|| serde::de::Error::missing_field("left"))?;
                let path = path.ok_or_else(|| serde::de::Error::missing_field("path"))?;
                Ok(PathRight::new(left, path, Default::default()))
            }
        }

        const FIELDS: &'static [&'static str] = &["path", "left"];
        deserializer.deserialize_struct("PathRight", FIELDS, PathRightVisitor)
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, PartialEq, Debug, Getters)]
pub struct PathLeft {
    #[get = "pub"]
    path: Path,
    #[get = "pub"]
    right: Hash,
    /// Depth (lenght) of the path
    #[serde(skip_serializing)]
    depth: Option<RecursiveDataSize>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(PathLeft, body);
has_encoding!(PathLeft, PATH_LEFT_ENCODING, {
    Encoding::Obj(vec![
        Field::new("path", PathCodec::get_encoding()),
        Field::new("right", Encoding::Hash(HashType::OperationListListHash)),
    ])
});

impl PathLeft {
    pub fn new(path: Path, right: Hash, body: BinaryDataCache) -> Self {
        let depth = path.depth().map(|d| d.checked_add(1)).flatten();
        Self {
            path,
            right,
            depth,
            body,
        }
    }
}

/// Custom deserialization is needed to properly set `depth` field.
/// See https://serde.rs/deserialize-struct.html
impl<'de> Deserialize<'de> for PathLeft {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        use serde::de::{self, Error, SeqAccess, Visitor};
        use std::fmt;
        enum Field {
            Path,
            Right,
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Field, D::Error>
            where
                D: de::Deserializer<'de>,
            {
                struct FieldVisitor;

                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;

                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`path` or `right`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "path" => Ok(Field::Path),
                            "right" => Ok(Field::Right),
                            _ => Err(Error::unknown_field(value, FIELDS)),
                        }
                    }
                }

                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PathLeftVisitor;

        impl<'de> Visitor<'de> for PathLeftVisitor {
            type Value = PathLeft;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct PathLeft")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<PathLeft, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let path = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let right = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(PathLeft::new(path, right, Default::default()))
            }

            fn visit_map<V>(self, mut map: V) -> Result<PathLeft, V::Error>
            where
                V: de::MapAccess<'de>,
            {
                let mut path = None;
                let mut right = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Path => {
                            if path.is_some() {
                                return Err(de::Error::duplicate_field("path"));
                            }
                            path = Some(map.next_value()?);
                        }
                        Field::Right => {
                            if right.is_some() {
                                return Err(de::Error::duplicate_field("right"));
                            }
                            right = Some(map.next_value()?);
                        }
                    }
                }
                let path = path.ok_or_else(|| serde::de::Error::missing_field("path"))?;
                let right = right.ok_or_else(|| serde::de::Error::missing_field("right"))?;
                Ok(PathLeft::new(path, right, Default::default()))
            }
        }

        const FIELDS: &'static [&'static str] = &["path", "right"];
        deserializer.deserialize_struct("PathLeft", FIELDS, PathLeftVisitor)
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Deserialize, PartialEq, Debug)]
pub enum Path {
    Left(Box<PathLeft>),
    Right(Box<PathRight>),
    Op,
}

impl Path {
    fn depth(&self) -> Option<RecursiveDataSize> {
        match self {
            Path::Left(l) => l.depth,
            Path::Right(r) => r.depth,
            Path::Op => Some(0),
        }
    }
}

/// Manual serializization ensures that path depth does not exceed max value
impl Serialize for Path {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self.depth() {
            Some(d) if d <= PATH_MAX_DEPTH => (),
            _ => {
                use serde::ser::Error;
                return Err(Error::custom("Path is too deep"));
            }
        }
        match self {
            Path::Left(h) => serializer.serialize_newtype_variant("Path", 0, "Left", h),
            Path::Right(h) => serializer.serialize_newtype_variant("Path", 1, "Right", h),
            Path::Op => serializer.serialize_unit_variant("Path", 2, "Op"),
        }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct GetOperationsForBlocksMessage {
    #[get = "pub"]
    get_operations_for_blocks: Vec<OperationsForBlock>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl GetOperationsForBlocksMessage {
    pub fn new(get_operations_for_blocks: Vec<OperationsForBlock>) -> Self {
        GetOperationsForBlocksMessage {
            get_operations_for_blocks,
            body: Default::default(),
        }
    }
}

cached_data!(GetOperationsForBlocksMessage, body);
has_encoding!(
    GetOperationsForBlocksMessage,
    GET_OPERATIONS_FOR_BLOCKS_MESSAGE_ENCODING,
    {
        Encoding::Obj(vec![Field::new(
            "get_operations_for_blocks",
            Encoding::dynamic(Encoding::list(OperationsForBlock::encoding().clone())),
        )])
    }
);

// ---------------------------------------

/// Custom encoder/decoder for [Path]
pub struct PathCodec {}

impl PathCodec {
    pub fn get_encoding() -> Encoding {
        Encoding::Custom(Arc::new(Self::new()))
    }

    fn new() -> Self {
        PathCodec {}
    }

    fn value_to_u8(value: &Value, encoding: &Encoding) -> Result<u8, Error> {
        match value {
            Value::Uint8(u) => Ok(*u),
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn list_to_u8s(value: &Value, encoding: &Encoding) -> Result<Vec<u8>, Error> {
        match value {
            Value::List(l) => l.iter().map(|i| Self::value_to_u8(i, encoding)).collect(),
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn encode_bytes(data: &mut Vec<u8>, value: &Value, encoding: &Encoding) -> Result<(), Error> {
        let bytes = Self::list_to_u8s(value, encoding)?;
        bytes.into_iter().for_each(|b| data.push(b));
        Ok(())
    }

    fn json_bytes(
        json_writer: &mut JsonWriter,
        value: &Value,
        encoding: &Encoding,
    ) -> Result<(), Error> {
        let bytes = Self::list_to_u8s(value, encoding)?;
        json_writer.open_array();
        let mut it = bytes.into_iter();
        if let Some(b) = it.next() {
            json_writer.push_num(b);
            it.for_each(|b| {
                json_writer.push_delimiter();
                json_writer.push_num(b);
            })
        }
        json_writer.close_array();
        Ok(())
    }

    fn encode_left<'a>(
        data: &mut Vec<u8>,
        tail: &mut VecDeque<Vec<u8>>,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<&'a Value>, Error> {
        match value {
            Value::Record(fields)
                if fields.len() == 2 && fields[0].0 == "path" && fields[1].0 == "right" =>
            {
                data.push(0xF0);
                let mut hash = Vec::new();
                Self::encode_bytes(&mut hash, &fields[1].1, encoding)?;
                tail.push_front(hash);
                Ok(Some(&fields[0].1))
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn json_left<'a>(
        json_writer: &mut JsonWriter,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<(&'a Value, Option<&'a Value>)>, Error> {
        match value {
            Value::Record(fields)
                if fields.len() == 2 && fields[0].0 == "path" && fields[1].0 == "right" =>
            {
                json_writer.open_record();
                json_writer.push_key("path");
                json_writer.open_record();

                Ok(Some((&fields[0].1, Some(&fields[1].1))))
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn encode_right<'a>(
        data: &mut Vec<u8>,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<&'a Value>, Error> {
        match value {
            Value::Record(fields)
                if fields.len() == 2 && fields[0].0 == "left" && fields[1].0 == "path" =>
            {
                data.push(0x0F);
                Self::encode_bytes(data, &fields[0].1, encoding)?;
                Ok(Some(&fields[1].1))
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn json_right<'a>(
        json_writer: &mut JsonWriter,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<(&'a Value, Option<&'a Value>)>, Error> {
        match value {
            Value::Record(fields)
                if fields.len() == 2 && fields[0].0 == "left" && fields[1].0 == "path" =>
            {
                json_writer.open_record();
                json_writer.push_key("left");
                Self::json_bytes(json_writer, &fields[0].1, encoding)?;
                json_writer.push_delimiter();
                json_writer.push_key("path");
                json_writer.open_record();
                Ok(Some((&fields[1].1, None)))
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn encode_op<'a>(data: &mut Vec<u8>) -> Result<Option<&'a Value>, Error> {
        data.push(0x00);
        Ok(None)
    }

    fn json_op<'a>(
        json_writer: &mut JsonWriter,
    ) -> Result<Option<(&'a Value, Option<&'a Value>)>, Error> {
        json_writer.push_str("Op");
        Ok(None)
    }

    fn encode_path<'a>(
        data: &mut Vec<u8>,
        tail: &mut VecDeque<Vec<u8>>,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<&'a Value>, Error> {
        match value {
            Value::Tag(name, inner) if name == "Left" => {
                Self::encode_left(data, tail, inner, encoding)
            }
            Value::Tag(name, inner) if name == "Right" => Self::encode_right(data, inner, encoding),
            Value::Enum(Some(name), Some(ord)) if name == "Op" && *ord == 2 => {
                Self::encode_op(data)
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn json_path<'a>(
        json_writer: &mut JsonWriter,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<(&'a Value, Option<&'a Value>)>, Error> {
        match value {
            Value::Tag(name, inner) if name == "Left" => {
                Self::json_left(json_writer, inner, encoding)
            }
            Value::Tag(name, inner) if name == "Right" => {
                Self::json_right(json_writer, inner, encoding)
            }
            Value::Enum(Some(name), Some(ord)) if name == "Op" && *ord == 2 => {
                Self::json_op(json_writer)
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn mk_op() -> Value {
        Value::Tag("Op".to_string(), Box::new(Value::Unit))
    }

    fn mk_list(bytes: &[u8]) -> Value {
        Value::List(bytes.iter().map(|b| Value::Uint8(*b)).collect())
    }

    fn mk_left(path: Value, right: &[u8]) -> Value {
        Value::Tag(
            "Left".to_string(),
            Box::new(Value::Record(vec![
                ("path".to_string(), path),
                ("right".to_string(), Self::mk_list(right)),
            ])),
        )
    }

    fn mk_right(left: &[u8], path: Value) -> Value {
        Value::Tag(
            "Right".to_string(),
            Box::new(Value::Record(vec![
                ("left".to_string(), Self::mk_list(left)),
                ("path".to_string(), path),
            ])),
        )
    }
}

impl CustomCodec for PathCodec {
    fn encode(
        &self,
        data: &mut Vec<u8>,
        mut value: &Value,
        encoding: &Encoding,
    ) -> Result<usize, Error> {
        let prev_size = data.len();
        let mut tail = VecDeque::new();
        while let Some(inner) = Self::encode_path(data, &mut tail, value, encoding)? {
            value = inner;
        }
        data.extend(tail.into_iter().flatten());
        data.len()
            .checked_sub(prev_size)
            .ok_or_else(|| Error::encoding_mismatch(encoding, value))
    }

    fn encode_json(
        &self,
        json_writer: &mut tezos_encoding::json_writer::JsonWriter,
        mut value: &Value,
        encoding: &Encoding,
    ) -> Result<(), Error> {
        let mut tails = VecDeque::new();
        while let Some((inner, tail)) = Self::json_path(json_writer, value, encoding)? {
            value = inner;
            tails.push_front(tail);
        }
        for tail in tails {
            match tail {
                Some(value) => {
                    json_writer.push_delimiter();
                    json_writer.push_key("right");
                    Self::json_bytes(json_writer, value, encoding)?;
                }
                _ => {}
            }
            json_writer.close_record();
            json_writer.close_record();
        }
        Ok(())
    }

    fn decode(&self, buf: &mut dyn Buf, _encoding: &Encoding) -> Result<Value, BinaryReaderError> {
        let mut hash = [0; HashType::OperationListListHash.size()];
        enum PathNode {
            Left,
            Right(Hash),
        }
        let mut nodes = Vec::new();
        loop {
            match safe!(buf, get_u8, u8) {
                0xF0 => {
                    nodes.push(PathNode::Left);
                }
                0x0F => {
                    safe!(buf, hash.len(), buf.copy_to_slice(&mut hash));
                    nodes.push(PathNode::Right(hash.to_vec()));
                }
                0x00 => {
                    let mut acc = Self::mk_op();
                    for node in nodes.iter().rev() {
                        match node {
                            PathNode::Left => {
                                safe!(buf, hash.len(), buf.copy_to_slice(&mut hash));
                                acc = Self::mk_left(acc, &hash);
                            }
                            PathNode::Right(hash) => {
                                acc = Self::mk_right(&hash, acc);
                            }
                        }
                    }
                    return Ok(acc);
                }
                t => {
                    return Err(BinaryReaderError::UnsupportedTag { tag: t as u16 });
                }
            }
            if nodes.len() > PATH_MAX_DEPTH as usize {
                return Err(BinaryReaderError::RecursiveDataOverflow {
                    name: "Path".to_string(),
                    max: PATH_MAX_DEPTH,
                });
            }
        }
    }
}
