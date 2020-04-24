// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::fmt;
use std::mem::size_of;

use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding, Tag, TagMap};

use crate::p2p::binary_message::cache::{CachedData, CacheReader, CacheWriter, NeverCache};

static DUMMY_BODY_CACHE: NeverCache = NeverCache;

#[derive(Serialize, Deserialize, PartialEq, Debug)]
pub enum AckMessage {
    Ack,
    NackV0,
    Nack(NackInfo),
}

#[derive(Serialize, Deserialize, PartialEq)]
pub struct NackInfo {
    pub motive: i16,
    pub potential_peers_to_connect: Vec<String>,
}

impl fmt::Debug for NackInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let motive = match self.motive {
            0 => "No_motive".to_string(),
            1 => "Too_many_connections ".to_string(),
            2 => "Unknown_chain_name".to_string(),
            3 => "Deprecated_p2p_version".to_string(),
            4 => "Deprecated_distributed_db_version".to_string(),
            5 => "Already_connected".to_string(),
            error_code => format!("Unknown_motive: {}", error_code),
        };
        let potential_peers_to_connect = self.potential_peers_to_connect.join(", ");
        write!(f, "motive: {}, potential_peers_to_connect: {:?}", motive, potential_peers_to_connect)
    }
}

impl NackInfo {
    fn encoding() -> Encoding {
        Encoding::Obj(
            vec![
                Field::new("motive", Encoding::Int16),
                Field::new("potential_peers_to_connect", Encoding::dynamic(Encoding::list(Encoding::String))),
            ]
        )
    }
}


impl HasEncoding for AckMessage {
    fn encoding() -> Encoding {
        Encoding::Tags(
            size_of::<u8>(),
            TagMap::new(&[
                Tag::new(0x00, "Ack", Encoding::Unit),
                Tag::new(0x01, "Nack", NackInfo::encoding()),
                Tag::new(0xFF, "NackV0", Encoding::Unit),
            ]),
        )
    }
}

impl CachedData for AckMessage {
    fn cache_reader(&self) -> &dyn CacheReader {
        &DUMMY_BODY_CACHE
    }

    fn cache_writer(&mut self) -> Option<&mut dyn CacheWriter> {
        None
    }
}