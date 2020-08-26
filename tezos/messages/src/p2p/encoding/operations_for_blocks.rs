// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::Arc;

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, Hash, HashType};
use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding, Tag, TagMap, TagVariant};
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
}

cached_data!(OperationsForBlock, body);
has_encoding!(OperationsForBlock, OPERATIONS_FOR_BLOCK_ENCODING, {
        Encoding::Obj(vec![
            Field::new(FieldName::Hash, Encoding::Hash(HashType::BlockHash)),
            Field::new(FieldName::ValidationPass, Encoding::Int8),
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
    pub operations: Vec<Operation>,
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl OperationsForBlocksMessage {
    pub fn new(operations_for_block: OperationsForBlock, operation_hashes_path: Path, operations: Vec<Operation>) -> Self {
        OperationsForBlocksMessage {
            operations_for_block,
            operation_hashes_path,
            operations,
            body: Default::default(),
        }
    }
}

cached_data!(OperationsForBlocksMessage, body);
has_encoding!(OperationsForBlocksMessage, OPERATIONS_FOR_BLOCKS_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new(FieldName::OperationsForBlock, OperationsForBlock::encoding().clone()),
            Field::new(FieldName::OperationHashesPath, path_encoding()),
            Field::new(FieldName::Operations, Encoding::list(Encoding::dynamic(Operation::encoding().clone()))),
        ])
});

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
            Field::new(FieldName::Left, Encoding::Hash(HashType::OperationListListHash)),
            Field::new(FieldName::Path, path_encoding()),
        ])
});

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
            Field::new(FieldName::Path, path_encoding()),
            Field::new(FieldName::Right, Encoding::Hash(HashType::OperationListListHash)),
        ])
});

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
        TagMap::new(&[
            Tag::new(0xF0, TagVariant::Left, Encoding::Lazy(Arc::new(|| PathLeft::encoding().clone()))),
            Tag::new(0x0F, TagVariant::Right, Encoding::Lazy(Arc::new(|| PathRight::encoding().clone()))),
            Tag::new(0x00, TagVariant::Op, Encoding::Unit),
        ])
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
has_encoding!(GetOperationsForBlocksMessage, GET_OPERATIONS_FOR_BLOCKS_MESSAGE_ENCODING, {
        Encoding::Obj(vec![
            Field::new(FieldName::GetOperationsForBlocks, Encoding::dynamic(Encoding::list(OperationsForBlock::encoding().clone()))),
        ])
});