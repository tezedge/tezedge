// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::sync::Arc;

use getset::{CopyGetters, Getters};
use nom::{
    branch::alt,
    bytes::complete::{tag, take},
    combinator::{flat_map, into, map, success, verify},
    multi::many_till,
    sequence::preceded,
};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, Hash, HashType};
use tezos_encoding::nom::NomReader;
use tezos_encoding::nom::NomResult;
use tezos_encoding::ser::Error;
use tezos_encoding::types::Value;
use tezos_encoding::{
    encoding::{CustomCodec, Encoding, HasEncoding},
    has_encoding,
};

use crate::p2p::encoding::operation::Operation;

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

#[derive(
    Clone, Serialize, Deserialize, PartialEq, Debug, CopyGetters, Getters, HasEncoding, NomReader,
)]
pub struct OperationsForBlock {
    #[get = "pub"]
    hash: BlockHash,
    #[get_copy = "pub"]
    validation_pass: i8,
}

impl OperationsForBlock {
    pub fn new(hash: BlockHash, validation_pass: i8) -> Self {
        OperationsForBlock {
            hash,
            validation_pass,
        }
    }

    /// alternative getter because .hash() causes problem with hash() method from Hash trait
    #[inline(always)]
    pub fn block_hash(&self) -> &BlockHash {
        &self.hash
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters, HasEncoding, NomReader)]
pub struct OperationsForBlocksMessage {
    #[get = "pub"]
    operations_for_block: OperationsForBlock,
    #[get = "pub"]
    operation_hashes_path: Path,
    #[get = "pub"]
    #[encoding(bounded = "OPERATION_LIST_MAX_SIZE", list, dynamic)]
    operations: Vec<Operation>,
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
        }
    }
}

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
}

impl PathRight {
    pub fn new(left: Hash) -> Self {
        Self { left }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct PathLeft {
    #[get = "pub"]
    right: Hash,
}

impl PathLeft {
    pub fn new(right: Hash) -> Self {
        Self { right }
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
        PathItem::Right(PathRight::new(left))
    }
    pub fn left(right: Hash) -> PathItem {
        PathItem::Left(PathLeft::new(right))
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, PartialEq, Debug, Deserialize)]
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
        self.0.iter().try_for_each(|i| seq.serialize_element(i))?;
        seq.end()
    }
}

has_encoding!(Path, PATH_ENCODING, { PathCodec::get_encoding() });

#[derive(Clone)]
enum DecodePathNode {
    Left,
    Right(Hash),
}

impl From<Vec<u8>> for DecodePathNode {
    fn from(bytes: Vec<u8>) -> Self {
        DecodePathNode::Right(bytes)
    }
}

fn hash(input: &[u8]) -> NomResult<Vec<u8>> {
    map(
        take(HashType::OperationListListHash.size()),
        |slice: &[u8]| slice.to_vec(),
    )(input)
}

fn path_left(input: &[u8]) -> NomResult<DecodePathNode> {
    preceded(tag(0xf0u8.to_be_bytes()), success(DecodePathNode::Left))(input)
}

fn path_right(input: &[u8]) -> NomResult<DecodePathNode> {
    preceded(tag(0x0fu8.to_be_bytes()), into(hash))(input)
}

fn path_op(input: &[u8]) -> NomResult<()> {
    preceded(tag(0x00u8.to_be_bytes()), success(()))(input)
}

fn path_complete(nodes: Vec<DecodePathNode>) -> impl FnMut(&[u8]) -> NomResult<Path> {
    move |mut input| {
        let mut res = Vec::new();
        for node in nodes.clone().into_iter().rev() {
            match node {
                DecodePathNode::Left => {
                    let (i, h) = hash(input)?;
                    res.push(PathItem::left(h));
                    input = i;
                }
                DecodePathNode::Right(h) => res.push(PathItem::right(h)),
            }
        }
        res.reverse();
        Ok((input, Path(res)))
    }
}

impl NomReader for Path {
    fn nom_read(bytes: &[u8]) -> tezos_encoding::nom::NomResult<Self> {
        flat_map(
            verify(
                map(many_till(alt((path_left, path_right)), path_op), |(v, _)| v),
                |nodes: &Vec<DecodePathNode>| {
                    MAX_PASS_MERKLE_DEPTH
                        .map(|v| v >= nodes.len())
                        .unwrap_or(true)
                },
            ),
            path_complete,
        )(bytes)
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct GetOperationsForBlocksMessage {
    #[get = "pub"]
    #[encoding(dynamic, list = "GET_OPERATIONS_FOR_BLOCKS_MAX_LENGTH")]
    get_operations_for_blocks: Vec<OperationsForBlock>,
}

impl GetOperationsForBlocksMessage {
    pub fn new(get_operations_for_blocks: Vec<OperationsForBlock>) -> Self {
        GetOperationsForBlocksMessage {
            get_operations_for_blocks,
        }
    }
}

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
}
