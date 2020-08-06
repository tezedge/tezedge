// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use getset::Getters;

use tezos_encoding::{
    encoding::{Encoding, Field, HasEncoding},
    has_encoding,
    types::BigInt,
};

use crate::p2p::binary_message::cache::{CachedData, CacheReader, CacheWriter, NeverCache};

static DUMMY_BODY_CACHE: NeverCache = NeverCache;

#[derive(Serialize, Deserialize, Debug, Clone, Getters)]
pub struct Counter {
    #[get = "pub"]
    counter: BigInt,
}

impl Counter {
    pub fn to_string(&self) -> String {
        self.counter.0.to_str_radix(10)
    }
}

impl CachedData for Counter {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &DUMMY_BODY_CACHE
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        None
    }
}

has_encoding!(Counter, COUNTER_ENCODING, {
        Encoding::Obj(vec![
            Field::new("counter", Encoding::Z)
        ])
});
