// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::sync::Arc;

use failure::Error;

use crypto::hash::HashType;
use storage::persistent::{Decoder, Encoder};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

#[test]
fn block_header_with_hash_encoded_equals_decoded() -> Result<(), Error> {
    let expected = BlockHeaderWithHash {
        hash: HashType::BlockHash
            .b58check_to_hash("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
        header: Arc::new(
            BlockHeaderBuilder::default()
                .level(34)
                .proto(1)
                .predecessor(
                    HashType::BlockHash
                        .b58check_to_hash("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
                )
                .timestamp(5_635_634)
                .validation_pass(4)
                .operations_hash(
                    HashType::OperationListListHash.b58check_to_hash(
                        "LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc",
                    )?,
                )
                .fitness(vec![vec![0, 0]])
                .context(
                    HashType::ContextHash
                        .b58check_to_hash("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd")?,
                )
                .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
                .build()
                .unwrap(),
        ),
    };
    let encoded_bytes = expected.encode()?;
    let decoded = BlockHeaderWithHash::decode(&encoded_bytes)?;
    Ok(assert_eq!(expected, decoded))
}
