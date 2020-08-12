// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::{CopyGetters, Getters};
use serde::{Serialize, Deserialize};

use crypto::hash::{BlockHash, HashType, OperationHash};
use tezos_encoding::encoding::{Encoding, Field, FieldName, HasEncoding};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
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

impl HasEncoding for GetOperationHashesForBlocksMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::GetOperationHashesForBlocks, Encoding::dynamic(Encoding::list(OperationHashesForBlock::encoding()))),
        ])
    }
}

impl CachedData for GetOperationHashesForBlocksMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

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
    pub fn new(operation_hashes_for_block: OperationHashesForBlock, operation_hashes_path: Path, operation_hashes: Vec<OperationHash>) -> Self {
        Self {
            operation_hashes_for_block,
            operation_hashes_path,
            operation_hashes,
            body: Default::default(),
        }
    }
}

impl HasEncoding for OperationHashesForBlocksMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::OperationHashesForBlock, OperationHashesForBlock::encoding()),
            Field::new(FieldName::OperationHashesPath, path_encoding()),
            Field::new(FieldName::OperationHashes, Encoding::list(Encoding::dynamic(Encoding::list(Encoding::Uint8)))),
        ])
    }
}

impl CachedData for OperationHashesForBlocksMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

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

impl HasEncoding for OperationHashesForBlock {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new(FieldName::Hash, Encoding::Hash(HashType::BlockHash)),
            Field::new(FieldName::ValidationPass, Encoding::Int8),
        ])
    }
}

impl CachedData for OperationHashesForBlock {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}