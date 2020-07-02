// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::mem::size_of;
use std::sync::Arc;
use std::collections::HashMap;
use std::string::ToString;

use serde::{Deserialize, Serialize};
use getset::Getters;
use failure::bail;

use crate::base::signature_public_key_hash::SignaturePublicKeyHash;
use crate::base::signature_public_key::SignaturePublicKey;
use crate::protocol::{ToRpcJsonMap, UniversalValue};

use tezos_encoding::{
    encoding::{Encoding, Field, HasEncoding},
    types::BigInt,
};

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, Debug, Clone, Getters)]
pub struct Counter {
    // TODO: merge with Code
    #[get = "pub"]
    counter: BigInt,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl Counter {
    pub fn to_string(&self) -> String {
        self.counter.0.to_str_radix(10)
    }
}

impl CachedData for Counter {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}

impl HasEncoding for Counter {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("counter", Encoding::Z)
        ])
    }
}
