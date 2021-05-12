// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use getset::Getters;
use serde::{Deserialize, Serialize};

use crypto::hash::{HashType, OperationHash};
use tezos_encoding::encoding::{Encoding, Field, HasEncoding};
use tezos_encoding::has_encoding_test;
use tezos_encoding::nom::NomReader;

use crate::cached_data;
use crate::p2p::binary_message::cache::BinaryDataCache;

use super::limits::MEMPOOL_MAX_SIZE;

#[derive(Clone, Serialize, Deserialize, Debug, Default, Getters, HasEncoding, NomReader, PartialEq)]
#[encoding(bounded = "MEMPOOL_MAX_SIZE")]
pub struct Mempool {
    #[get = "pub"]
    #[encoding(dynamic, list)]
    known_valid: Vec<OperationHash>,
    #[get = "pub"]
    #[encoding(dynamic, dynamic, list)]
    pending: Vec<OperationHash>,
    #[serde(skip_serializing)]
    #[encoding(skip)]
    body: BinaryDataCache,
}

impl Mempool {
    pub fn new(known_valid: Vec<OperationHash>, pending: Vec<OperationHash>) -> Self {
        Mempool {
            known_valid,
            pending,
            body: Default::default(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.known_valid.is_empty() && self.pending.is_empty()
    }
}

cached_data!(Mempool, body);
has_encoding_test!(Mempool, MEMPOOL_ENCODING, {
    Encoding::bounded(
        MEMPOOL_MAX_SIZE,
        Encoding::Obj(
            "Mempool",
            vec![
                Field::new(
                    "known_valid",
                    Encoding::dynamic(Encoding::list(Encoding::Hash(HashType::OperationHash))),
                ),
                Field::new(
                    "pending",
                    Encoding::dynamic(Encoding::dynamic(Encoding::list(Encoding::Hash(
                        HashType::OperationHash,
                    )))),
                ),
            ],
        ),
    )
});


#[cfg(test)]
mod test {
    use tezos_encoding::assert_encodings_match;

    #[test]
    fn test_mempool_encoding_schema() {
        assert_encodings_match!(super::Mempool);
    }
}
