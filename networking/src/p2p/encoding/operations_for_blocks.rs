use std::mem::size_of;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, TagMap, Tag};
use tezos_encoding::hash::{BlockHash, HashEncoding, HashType, Hash};
use crate::p2p::encoding::operation::Operation;
use std::rc::Rc;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct OperationsForBlock {
    pub hash: BlockHash,
    pub validation_pass: i8
}

impl HasEncoding for OperationsForBlock {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("hash", Encoding::Hash(HashEncoding::new(HashType::BlockHash))),
            Field::new("validation_pass", Encoding::Int8),
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct OperationsForBlocksMessage {
    pub operations_for_block: OperationsForBlock,
    pub operation_hashes_path: Path,
    pub operations: Vec<Operation>
}

impl HasEncoding for OperationsForBlocksMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("operations_for_block", OperationsForBlock::encoding()),
            Field::new("operation_hashes_path", path_encoding()),
            Field::new("operations", Encoding::list(Encoding::dynamic(Operation::encoding()))),
        ])
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct PathRight {
    pub left: Hash,
    pub path: Path,
}

impl HasEncoding for PathRight {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("left", Encoding::Hash(HashEncoding::new(HashType::OperationListListHash))),
            Field::new("path", path_encoding()),
        ])
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct PathLeft {
    pub path: Path,
    pub right: Hash
}

impl HasEncoding for PathLeft {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("path", path_encoding()),
            Field::new("right", Encoding::Hash(HashEncoding::new(HashType::OperationListListHash))),
        ])
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub enum Path {
    Left(Box<PathLeft>),
    Right(Box<PathRight>),
    Op
}

fn path_encoding() -> Encoding {
    Encoding::Tags(
        size_of::<u8>(),
        TagMap::new(&[
            Tag::new(0xF0, "Left", Encoding::Lazy(Rc::new(PathLeft::encoding))),
            Tag::new(0x0F, "Right", Encoding::Lazy(Rc::new(PathRight::encoding))),
            Tag::new(0x00, "Op", Encoding::Unit),
        ])
    )
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetOperationsForBlocksMessage {
    pub get_operations_for_blocks: Vec<OperationsForBlock>
}

impl HasEncoding for GetOperationsForBlocksMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_operations_for_blocks", Encoding::dynamic(Encoding::list(OperationsForBlock::encoding()))),
        ])
    }
}
