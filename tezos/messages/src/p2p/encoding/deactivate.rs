// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{ChainId, HashType};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding_test;
use tezos_encoding::nom::NomReader;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

#[derive(Serialize, Deserialize, Debug, Getters, Clone, HasEncoding, NomReader)]
pub struct DeactivateMessage {
    #[get = "pub"]
    deactivate: ChainId,

    #[serde(skip_serializing)]
    #[encoding(skip)]
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
has_encoding_test!(DeactivateMessage, DEACTIVATE_MESSAGE_ENCODING, {
    Encoding::Obj(
        "DeactivateMessage",
        vec![Field::new("deactivate", Encoding::Hash(HashType::ChainId))],
    )
});

#[cfg(test)]
mod test {
    use tezos_encoding::assert_encodings_match;

    use super::*;

    #[test]
    fn test_deactivate_encoding_schema() {
        assert_encodings_match!(DeactivateMessage);
    }
}
