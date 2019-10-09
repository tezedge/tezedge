// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use derive_new::new;
use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{ChainId, HashEncoding, HashType};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

use super::block_header::BlockHeader;
use super::mempool::Mempool;

#[derive(Serialize, Deserialize, Debug, Getters, new)]
pub struct CurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,
    #[get = "pub"]
    current_block_header: BlockHeader,
    #[get = "pub"]
    #[new(default)]
    current_mempool: Mempool,
    #[serde(skip_serializing)]
    #[new(default)]
    body: BinaryDataCache,
}

impl HasEncoding for CurrentHeadMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId))),
            Field::new("current_block_header", Encoding::dynamic(BlockHeader::encoding())),
            Field::new("current_mempool", Mempool::encoding())
        ])
    }
}

impl CachedData for CurrentHeadMessage {
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
#[derive(Serialize, Deserialize, Debug, Getters, new)]
pub struct GetCurrentHeadMessage {
    #[get = "pub"]
    chain_id: ChainId,

    #[serde(skip_serializing)]
    #[new(default)]
    body: BinaryDataCache,
}

impl HasEncoding for GetCurrentHeadMessage {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("chain_id", Encoding::Hash(HashEncoding::new(HashType::ChainId)))
        ])
    }
}

impl CachedData for GetCurrentHeadMessage {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}