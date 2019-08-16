use std::mem::size_of;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, TagMap, Tag};
use tezos_encoding::hash::{BlockHash, HashEncoding, HashType, Hash};
use crate::p2p::encoding::operation::Operation;
use std::rc::Rc;

#[derive(Serialize, Deserialize, Debug)]
pub struct OperationsForBlock {
    hash: BlockHash,
    validation_pass: i8
}

impl OperationsForBlock {
    pub fn hash(&self) -> &BlockHash {
        &self.hash
    }

    pub fn validation_pass(&self) -> i8 {
        self.validation_pass
    }
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
#[derive(Serialize, Deserialize, Debug)]
pub struct OperationsForBlocksMessage {
    operations_for_block: OperationsForBlock,
    operation_hashes_path: Path,
    operations: Vec<Operation>
}

impl OperationsForBlocksMessage {
    pub fn operations_for_block(&self) -> &OperationsForBlock {
        &self.operations_for_block
    }

    pub fn operation_hashes_path(&self) -> &Path {
        &self.operation_hashes_path
    }

    pub fn operations(&self) -> &Vec<Operation> {
        &self.operations
    }
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
#[derive(Serialize, Deserialize, Debug)]
pub struct PathRight {
    left: Hash,
    path: Path,
}

impl PathRight {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn left(&self) -> &Hash {
        &self.left
    }
}

impl HasEncoding for PathRight {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("left", Encoding::Hash(HashEncoding::new(HashType::OperationListListHash))),
            Field::new("path", path_encoding()),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PathLeft {
    path: Path,
    right: Hash
}

impl PathLeft {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn right(&self) -> &Hash {
        &self.right
    }
}

impl HasEncoding for PathLeft {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("path", path_encoding()),
            Field::new("right", Encoding::Hash(HashEncoding::new(HashType::OperationListListHash))),
        ])
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Path {
    Left(Box<PathLeft>),
    Right(Box<PathRight>),
    Op
}

fn path_encoding() -> Encoding {
    Encoding::Tags(
        size_of::<u8>(),
        TagMap::new(&[
            Tag::new(0xF0, "Left", Encoding::Lazy(Rc::new(|| PathLeft::encoding()))),
            Tag::new(0x0F, "Right", Encoding::Lazy(Rc::new(|| PathRight::encoding()))),
            Tag::new(0x00, "Op", Encoding::Unit),
        ])
    )
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetOperationsForBlocksMessage {
    get_operations_for_blocks: Vec<OperationsForBlock>
}

impl GetOperationsForBlocksMessage {
    pub fn get_operations_for_blocks(&self) -> &Vec<OperationsForBlock> {
        &self.get_operations_for_blocks
    }
}

impl HasEncoding for GetOperationsForBlocksMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("get_operations_for_blocks", Encoding::dynamic(Encoding::list(OperationsForBlock::encoding()))),
        ])
    }
}
