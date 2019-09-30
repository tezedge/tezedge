use std::mem::size_of;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, HasEncoding, Tag, TagMap};

use crate::p2p::binary_message::cache::{ CachedData, CacheReader, CacheWriter, NeverCache};

static DUMMY_BODY_CACHE: NeverCache = NeverCache;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum AckMessage {
    Ack,
    Nack,
}

impl HasEncoding for AckMessage {
    fn encoding() -> Encoding {
        Encoding::Tags(
            size_of::<u8>(),
            TagMap::new(&[
                Tag::new(0x00, "Ack", Encoding::Unit),
                Tag::new(0xFF, "Nack", Encoding::Unit),
            ])
        )
    }
}

impl CachedData for AckMessage {
    fn cache_reader(&self) -> & dyn CacheReader {
        &DUMMY_BODY_CACHE
    }

    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        None
    }
}