use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::hash::{HashEncoding, HashType, OperationHash};

use crate::p2p::binary_message::cache::{BinaryDataCache, CacheReader, CacheWriter, CachedData};

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Mempool {
    pub known_valid: Vec<OperationHash>,
    pub pending: Vec<OperationHash>,

    #[serde(skip_serializing)]
    pub body: BinaryDataCache,
}

impl HasEncoding for Mempool {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("known_valid", Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash))))),
            Field::new("pending", Encoding::dynamic(Encoding::dynamic(Encoding::list(Encoding::Hash(HashEncoding::new(HashType::OperationHash)))))),
        ])
    }
}

impl CachedData for Mempool {
    #[inline]
    fn cache_reader(&self) -> & dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}
