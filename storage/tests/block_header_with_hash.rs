// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use anyhow::Error;
use crypto::hash::{BlockHash, HashType};

use storage::persistent::{Decoder, Encoder};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::fitness::Fitness;
use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

#[test]
fn block_header_with_hash_encoded_equals_decoded() -> Result<(), Error> {
    let hash: BlockHash = "BL84RJX8tqB3WkFPWCcg1Lm6KYE5gns9UYFguihG5Yy17UwnL3b".try_into()?;
    let hash_bytes = hash.as_ref().to_vec();
    let expected = BlockHeaderWithHash::new(
        BlockHeaderBuilder::default()
            .level(34)
            .proto(1)
            .predecessor("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET".try_into()?)
            .timestamp(5_635_634.into())
            .validation_pass(4)
            .operations_hash("LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc".try_into()?)
            .fitness(Fitness::from(vec![vec![0, 0]]))
            .context("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd".try_into()?)
            .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8].into())
            .hash(hash_bytes.into())
            .build()
            .unwrap(),
    )?;
    let encoded_bytes = expected.encode()?;
    let decoded = BlockHeaderWithHash::decode(&encoded_bytes)?;
    assert_eq!(expected, decoded);
    Ok(())
}

#[test]
fn block_header_with_hash_decode_empty() {
    assert!(matches!(BlockHeaderWithHash::decode(&[]), Err(_)));
}

#[test]
fn block_header_with_hash_decode_incomplete_hash() {
    assert!(matches!(
        BlockHeaderWithHash::decode(&[0; HashType::BlockHash.size() - 1]),
        Err(_)
    ));
}

#[test]
fn block_header_with_hash_decode_no_header() {
    assert!(matches!(
        BlockHeaderWithHash::decode(&[0; HashType::BlockHash.size()]),
        Err(_)
    ));
}
