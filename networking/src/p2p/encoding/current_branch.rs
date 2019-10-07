// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use derive_new::new;
use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, SchemaType};
use tezos_encoding::hash::{BlockHash, ChainId, HashEncoding, HashType};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
use crate::p2p::encoding::block_header::BlockHeader;

#[derive(Clone, Serialize, Deserialize, Debug, Getters, new)]
pub struct CurrentBranchMessage {
    #[get = "pub"]
    chain_id: ChainId,
    #[get = "pub"]
    current_branch: CurrentBranch,
    #[new(default)]
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for CurrentBranchMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId))),
            Field::new("current_branch", CurrentBranch::encoding())
        ])
    }
}

impl CachedData for CurrentBranchMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Clone, Serialize, Deserialize, Debug, Getters, new)]
pub struct CurrentBranch {
    #[get = "pub"]
    current_head: BlockHeader,
    #[get = "pub"]
    #[new(default)]
    history: Vec<BlockHash>,
    #[new(default)]
    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl HasEncoding for CurrentBranch {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("current_head", Encoding::dynamic(BlockHeader::encoding())),
            Field::new("history", Encoding::Split(Arc::new(|schema_type|
                match schema_type {
                    SchemaType::Json => Encoding::Unit, // TODO: decode as list of hashes when history is needed
                    SchemaType::Binary => Encoding::list(Encoding::Hash(HashEncoding::new(HashType::BlockHash)))
                }
            )))
        ])
    }
}

impl CachedData for CurrentBranch {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

// -----------------------------------------------------------------------------------------------
#[derive(Serialize, Deserialize, Debug)]
pub struct GetCurrentBranchMessage {
    pub chain_id: ChainId,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl GetCurrentBranchMessage {
    pub fn new(chain_id: ChainId) -> Self {
        GetCurrentBranchMessage { chain_id, body: Default::default() }
    }
}

impl HasEncoding for GetCurrentBranchMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId)))
        ])
    }
}

impl CachedData for GetCurrentBranchMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}