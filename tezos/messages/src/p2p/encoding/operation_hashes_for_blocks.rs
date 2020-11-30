// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::{CopyGetters, Getters};
use serde::{Deserialize, Serialize};

use crypto::hash::{BlockHash, HashType, OperationHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;
use crate::p2p::encoding::prelude::Path;

use super::operations_for_blocks::path_encoding;

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct GetOperationHashesForBlocksMessage {
    #[get = "pub"]
    get_operation_hashes_for_blocks: Vec<OperationHashesForBlock>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl GetOperationHashesForBlocksMessage {
    pub fn new(get_operation_hashes_for_blocks: Vec<OperationHashesForBlock>) -> Self {
        Self {
            get_operation_hashes_for_blocks,
            body: Default::default(),
        }
    }
}

cached_data!(GetOperationHashesForBlocksMessage, body);
has_encoding!(
    GetOperationHashesForBlocksMessage,
    GET_OPERATION_HASHES_FOR_BLOCKS_MESSAGE_ENCODING,
    {
        Encoding::Obj(vec![Field::new(
            "get_operation_hashes_for_blocks",
            Encoding::dynamic(Encoding::list(OperationHashesForBlock::encoding().clone())),
        )])
    }
);

// ------------------ Response ------------------ //
#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct OperationHashesForBlocksMessage {
    #[get = "pub"]
    operation_hashes_for_block: OperationHashesForBlock,
    #[get = "pub"]
    operation_hashes_path: Path,
    #[get = "pub"]
    operation_hashes: Vec<OperationHash>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl OperationHashesForBlocksMessage {
    pub fn new(
        operation_hashes_for_block: OperationHashesForBlock,
        operation_hashes_path: Path,
        operation_hashes: Vec<OperationHash>,
    ) -> Self {
        Self {
            operation_hashes_for_block,
            operation_hashes_path,
            operation_hashes,
            body: Default::default(),
        }
    }
}

cached_data!(OperationHashesForBlocksMessage, body);
has_encoding!(
    OperationHashesForBlocksMessage,
    OPERATION_HASHES_FOR_BLOCKS_MESSAGE_ENCODING,
    {
        Encoding::Obj(vec![
            Field::new(
                "operation_hashes_for_block",
                OperationHashesForBlock::encoding().clone(),
            ),
            Field::new("operation_hashes_path", path_encoding()),
            Field::new(
                "operation_hashes",
                Encoding::list(Encoding::dynamic(Encoding::list(Encoding::Uint8))),
            ),
        ])
    }
);

// ------------------ Inner message for operation hashes message ------------------ //
#[derive(Serialize, Deserialize, Debug, Getters, CopyGetters, Clone)]
pub struct OperationHashesForBlock {
    #[get = "pub"]
    hash: BlockHash,
    #[get_copy = "pub"]
    validation_pass: i8,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl OperationHashesForBlock {
    pub fn new(hash: BlockHash, validation_pass: i8) -> Self {
        Self {
            hash,
            validation_pass,
            body: Default::default(),
        }
    }
}

cached_data!(OperationHashesForBlock, body);
has_encoding!(
    OperationHashesForBlock,
    OPERATION_HASHES_FOR_BLOCK_ENCODING,
    {
        Encoding::Obj(vec![
            Field::new("hash", Encoding::Hash(HashType::BlockHash)),
            Field::new("validation_pass", Encoding::Int8),
        ])
    }
);
