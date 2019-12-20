// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
use serde::{Serialize, Deserialize};
use crypto::hash::HashType;
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use crate::p2p::binary_message::cache::{BinaryDataCache, CachedData, CacheReader, CacheWriter};
use std::collections::HashMap;
use crate::protocol::UniversalValue;


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EndorsingRightEncode {
    level: i32,
    delegate: String,
    slots: Vec<u16>,
    estimated_time: String,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl EndorsingRightEncode {
    pub fn as_map(self) -> HashMap<&'static str, UniversalValue> {
        let mut ret: HashMap<&'static str, UniversalValue> = Default::default();
        ret.insert("level", UniversalValue::num_i32(self.level));
        ret.insert("delegate", UniversalValue::string(self.delegate));
        ret.insert("slots", UniversalValue::num_list_u16(self.slots.iter()));
        ret.insert("estimated_time", UniversalValue::string(self.estimated_time));
        ret
    }
}

impl HasEncoding for EndorsingRightEncode {
    fn encoding() -> Encoding {
        Encoding::Obj(vec![
            Field::new("level", Encoding::Int32),
            Field::new("delegate", Encoding::Hash(HashType::PublicKeyHash)),
            Field::new("slots", Encoding::dynamic(Encoding::list(Encoding::Uint16))),
            Field::new("estimated_time", Encoding::Timestamp)
        ])
    }
}

impl CachedData for EndorsingRightEncode {
    #[inline]
    fn cache_reader(&self) -> &dyn CacheReader {
        &self.body
    }

    #[inline]
    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        Some(&mut self.body)
    }
}