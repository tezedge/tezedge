// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{ChainId, HashType};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct DeactivateMessage {
    #[get = "pub"]
    deactivate: ChainId,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl DeactivateMessage {
    pub fn new(deactivate: ChainId) -> Self {
        Self {
            deactivate,
            body: Default::default(),
        }
    }
}

cached_data!(DeactivateMessage, body);
has_encoding!(DeactivateMessage, DEACTIVATE_MESSAGE_ENCODING, {
    Encoding::Obj(vec![Field::new(
        "deactivate",
        Encoding::Hash(HashType::ChainId),
    )])
});
