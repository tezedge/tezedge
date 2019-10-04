use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{ChainId, HashEncoding, HashType};

use crate::p2p::binary_message::cache::{BinaryDataCache, CacheReader, CacheWriter, CachedData};

use super::block_header::BlockHeader;
use super::mempool::Mempool;

#[derive(Serialize, Deserialize, Debug)]
pub struct CurrentHeadMessage {
    pub chain_id: ChainId,
    pub current_block_header: BlockHeader,
    pub current_mempool: Mempool,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl CurrentHeadMessage {
    pub fn new(chain_id: &ChainId, current_block_header: &BlockHeader) -> Self {
        CurrentHeadMessage {
            chain_id: chain_id.clone(),
            current_block_header: current_block_header.clone(),
            current_mempool: Default::default(),
            body: Default::default()
        }
    }
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
#[derive(Serialize, Deserialize, Debug)]
pub struct GetCurrentHeadMessage {
    pub chain_id: ChainId,

    #[serde(skip_serializing)]
    pub body: BinaryDataCache,
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