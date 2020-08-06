// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct AdvertiseMessage {
    #[get = "pub"]
    id: Vec<String>,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl AdvertiseMessage {
    pub fn new(addresses: &[SocketAddr]) -> Self {
        Self {
            id: addresses.iter().map(|address| format!("{}", address)).collect(),
            body: Default::default(),
        }
    }
}

has_encoding!(AdvertiseMessage, ADVERTISE_MESSAGE_ENCODING, {
    Encoding::Obj(vec![
        Field::new("id", Encoding::list(Encoding::String)),
    ])
});

impl CachedData for AdvertiseMessage {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}
