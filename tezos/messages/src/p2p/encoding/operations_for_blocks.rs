// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::sync::Arc;

use bytes::Buf;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, Hash, HashType};
use tezos_encoding::binary_reader::BinaryReaderError;
use tezos_encoding::encoding::{CustomCodec, Encoding, Field, HasEncoding};
use tezos_encoding::ser::Error;
use tezos_encoding::types::Value;
use tezos_encoding::{has_encoding, safe};

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;
use crate::p2p::encoding::operation::Operation;
use tezos_encoding::json_writer::JsonWriter;

use super::limits::{GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH, OPERATION_LIST_MAX_SIZE};

/// Maximal length for path in a Merkle tree for list of lists of operations.
/// This is calculated from Tezos limit on that Operation_list_list size:
///
/// `let operation_max_pass = ref (Some 8) (* FIXME: arbitrary *)`
///
/// See https://gitlab.com/simplestaking/tezos/-/blob/master/src/lib_shell/distributed_db_message.ml#L65
///
/// A  Merkle tree for that list of lenght 8 will be 3 (log_2(8)) levels at most,
/// thus any path should be 3 steps at most.
///
/// TODO: Implement mechanism for updating this, when Tezos implements this.
pub const MAX_PASS_MERKLE_DEPTH: Option<usize> = Some(3);

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
                Encoding::bounded(
                    OPERATION_LIST_MAX_SIZE,
                    Encoding::list(Encoding::dynamic(Operation::encoding().clone())),
                ),
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
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct PathRight {
    #[get = "pub"]
    left: Hash,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(PathRight, body);

impl PathRight {
    pub fn new(left: Hash, body: BinaryDataCache) -> Self {
        Self { left, body }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct PathLeft {
    #[get = "pub"]
    right: Hash,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(PathLeft, body);

impl PathLeft {
    pub fn new(right: Hash, body: BinaryDataCache) -> Self {
        Self { right, body }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum PathItem {
    Right(PathRight),
    Left(PathLeft),
}

impl PathItem {
    pub fn right(left: Hash) -> PathItem {
        PathItem::Right(PathRight::new(left, Default::default()))
    }
    pub fn left(right: Hash) -> PathItem {
        PathItem::Left(PathLeft::new(right, Default::default()))
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Deserialize, PartialEq, Debug)]
pub struct Path(pub Vec<PathItem>);

impl Path {
    pub fn op() -> Self {
        Path(Vec::new())
    }
}

/// Manual serializization ensures that path depth does not exceed max value
impl Serialize for Path {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match MAX_PASS_MERKLE_DEPTH {
            Some(max) => {
                if self.0.len() > max {
                    use serde::ser::Error;
                    return Err(Error::custom(format!(
                        "Path size exceedes its boundary {} for encoding",
                        max
                    )));
                }
            }
            _ => (),
        }
        use serde::ser::SerializeSeq;
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        self.0
            .iter()
            .map(|i| seq.serialize_element(i))
            .collect::<Result<_, _>>()?;
        seq.end()
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
            Encoding::dynamic(Encoding::bounded_list(
                GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH,
                OperationsForBlock::encoding().clone(),
            )),
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

    fn encode_left(
        data: &mut Vec<u8>,
        value: &Value,
        encoding: &Encoding,
    ) -> Result<Option<Vec<u8>>, Error> {
        match value {
            Value::Record(fields) if fields.len() == 1 && fields[0].0 == "right" => {
                data.push(0xF0);
                let mut hash = Vec::new();
                Self::encode_bytes(&mut hash, &fields[0].1, encoding)?;
                Ok(Some(hash))
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn json_left<'a>(
        json_writer: &mut JsonWriter,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<&'a Value>, Error> {
        match value {
            Value::Record(fields) if fields.len() == 2 && fields[0].0 == "right" => {
                json_writer.open_record();
                json_writer.push_key("path");
                json_writer.open_record();

                Ok(Some(&fields[0].1))
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn encode_right(
        data: &mut Vec<u8>,
        value: &Value,
        encoding: &Encoding,
    ) -> Result<Option<Vec<u8>>, Error> {
        match value {
            Value::Record(fields) if fields.len() == 1 && fields[0].0 == "left" => {
                data.push(0x0F);
                Self::encode_bytes(data, &fields[0].1, encoding)?;
                Ok(None)
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn json_right<'a>(
        json_writer: &mut JsonWriter,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<&'a Value>, Error> {
        match value {
            Value::Record(fields) if fields.len() == 2 && fields[0].0 == "left" => {
                json_writer.open_record();
                json_writer.push_key("left");
                Self::json_bytes(json_writer, &fields[0].1, encoding)?;
                json_writer.push_delimiter();
                json_writer.push_key("path");
                json_writer.open_record();
                Ok(None)
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn encode_path_item(
        data: &mut Vec<u8>,
        value: &Value,
        encoding: &Encoding,
    ) -> Result<Option<Vec<u8>>, Error> {
        match value {
            Value::Tag(name, inner) if name == "Left" => Self::encode_left(data, inner, encoding),
            Value::Tag(name, inner) if name == "Right" => Self::encode_right(data, inner, encoding),
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn json_path_item<'a>(
        json_writer: &mut JsonWriter,
        value: &'a Value,
        encoding: &Encoding,
    ) -> Result<Option<&'a Value>, Error> {
        match value {
            Value::Tag(name, inner) if name == "Left" => {
                Self::json_left(json_writer, inner, encoding)
            }
            Value::Tag(name, inner) if name == "Right" => {
                Self::json_right(json_writer, inner, encoding)
            }
            _ => Err(Error::encoding_mismatch(encoding, value)),
        }
    }

    fn mk_list(bytes: &[u8]) -> Value {
        Value::List(bytes.iter().map(|b| Value::Uint8(*b)).collect())
    }

    fn mk_left(right: &[u8]) -> Value {
        Value::Tag(
            "Left".to_string(),
            Box::new(Value::Record(vec![(
                "right".to_string(),
                Self::mk_list(right),
            )])),
        )
    }

    fn mk_right(left: &[u8]) -> Value {
        Value::Tag(
            "Right".to_string(),
            Box::new(Value::Record(vec![(
                "left".to_string(),
                Self::mk_list(left),
            )])),
        )
    }
}

impl CustomCodec for PathCodec {
    fn encode(
        &self,
        data: &mut Vec<u8>,
        value: &Value,
        encoding: &Encoding,
    ) -> Result<usize, Error> {
        if let Value::List(values) = value {
            let prev_size = data.len();
            let mut tails = VecDeque::new();
            for path_item in values {
                let tail = Self::encode_path_item(data, path_item, encoding)?;
                tails.push_front(tail);
            }
            data.push(0x00);
            data.extend(tails.into_iter().filter_map(|e| e).flatten());
            data.len()
                .checked_sub(prev_size)
                .ok_or_else(|| Error::encoding_mismatch(encoding, value))
        } else {
            Err(Error::encoding_mismatch(encoding, value))
        }
    }

    fn encode_json(
        &self,
        json_writer: &mut tezos_encoding::json_writer::JsonWriter,
        value: &Value,
        encoding: &Encoding,
    ) -> Result<(), Error> {
        if let Value::List(values) = value {
            let mut tails = VecDeque::new();
            for path_item in values {
                let tail = Self::json_path_item(json_writer, path_item, encoding)?;
                tails.push_front(tail);
            }
            json_writer.push_str("Op");
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
        } else {
            Err(Error::encoding_mismatch(encoding, value))
        }
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
                    let mut result = Vec::with_capacity(nodes.len());
                    for node in nodes.iter().rev() {
                        match node {
                            PathNode::Left => {
                                safe!(buf, hash.len(), buf.copy_to_slice(&mut hash));
                                result.push(Self::mk_left(&hash));
                            }
                            PathNode::Right(hash) => {
                                result.push(Self::mk_right(&hash));
                            }
                        }
                    }
                    result.reverse();
                    return Ok(Value::List(result));
                }
                t => {
                    return Err(BinaryReaderError::UnsupportedTag { tag: t as u16 });
                }
            }
            match MAX_PASS_MERKLE_DEPTH {
                Some(max) => {
                    if nodes.len() > max {
                        return Err(BinaryReaderError::EncodingBoundaryExceeded {
                            name: "Path".to_string(),
                            boundary: max,
                        });
                    }
                }
                _ => (),
            }
        }
    }
}
