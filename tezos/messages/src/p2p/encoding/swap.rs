// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crypto::hash::{CryptoboxPublicKeyHash, HashType};
use getset::Getters;
use serde::{Deserialize, Serialize};

use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::limits::P2P_POINT_MAX_LENGTH;

#[derive(Serialize, Deserialize, Debug, Getters, Clone)]
pub struct SwapMessage {
    #[get = "pub"]
    point: String,
    #[get = "pub"]
    peer_id: CryptoboxPublicKeyHash,

    #[serde(skip_serializing)]
    body: BinaryDataCache,
}

impl SwapMessage {
    pub fn new(point: String, peer_id: CryptoboxPublicKeyHash) -> Self {
        Self {
            point,
            peer_id,
            body: Default::default(),
        }
    }
}

cached_data!(SwapMessage, body);
has_encoding!(SwapMessage, SWAP_MESSAGE_ENCODING, {
    Encoding::Obj(vec![
        Field::new("point", Encoding::BoundedString(P2P_POINT_MAX_LENGTH)),
        Field::new("peer_id", Encoding::Hash(HashType::CryptoboxPublicKeyHash)),
    ])
});
