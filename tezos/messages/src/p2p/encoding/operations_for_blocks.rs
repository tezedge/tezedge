// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::Arc;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, Hash, HashType};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding, Tag, TagMap};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;
use crate::p2p::encoding::operation::Operation;

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
            Field::new("operation_hashes_path", path_encoding()),
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
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct PathRight {
    #[get = "pub"]
    left: Hash,
    #[get = "pub"]
    path: Path,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(PathRight, body);
has_encoding!(PathRight, PATH_RIGHT_ENCODING, {
    Encoding::Obj(vec![
        Field::new("left", Encoding::Hash(HashType::OperationListListHash)),
        Field::new("path", path_encoding()),
    ])
});

impl PathRight {
    pub fn new(left: Hash, path: Path, body: BinaryDataCache) -> Self {
        Self { left, path, body }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug, Getters)]
pub struct PathLeft {
    #[get = "pub"]
    path: Path,
    #[get = "pub"]
    right: Hash,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

cached_data!(PathLeft, body);
has_encoding!(PathLeft, PATH_LEFT_ENCODING, {
    Encoding::Obj(vec![
        Field::new("path", path_encoding()),
        Field::new("right", Encoding::Hash(HashType::OperationListListHash)),
    ])
});

impl PathLeft {
    pub fn new(path: Path, right: Hash, body: BinaryDataCache) -> Self {
        Self { path, right, body }
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum Path {
    Left(Box<PathLeft>),
    Right(Box<PathRight>),
    Op,
}

pub fn path_encoding() -> Encoding {
    Encoding::Tags(
        size_of::<u8>(),
        TagMap::new(vec![
            Tag::new(
                0xF0,
                "Left",
                Encoding::Lazy(Arc::new(|| PathLeft::encoding().clone())),
            ),
            Tag::new(
                0x0F,
                "Right",
                Encoding::Lazy(Arc::new(|| PathRight::encoding().clone())),
            ),
            Tag::new(0x00, "Op", Encoding::Unit),
        ]),
    )
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
