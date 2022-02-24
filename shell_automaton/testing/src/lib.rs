// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use crypto::{
    hash::CryptoboxPublicKeyHash,
    seeded_step::{Seed, Step},
};
use storage::BlockHeaderWithHash;
use tezos_messages::p2p::encoding::block_header::{BlockHeaderBuilder, Level};

pub mod one_real_node_cluster;
pub mod service;

pub fn generate_chain(
    genesis_block: BlockHeaderWithHash,
    target_level: Level,
) -> Vec<BlockHeaderWithHash> {
    (1..=target_level).fold(vec![genesis_block.clone()], |mut chain, level| {
        let pred = chain.last().unwrap_or(&genesis_block);
        let block = generate_next_block(pred, 0);
        chain.push(block);
        chain
    })
}

pub fn generate_next_block(
    pred: &BlockHeaderWithHash,
    additional_timestamp: i64,
) -> BlockHeaderWithHash {
    let level = pred.header.level() + 1;
    let block_header = BlockHeaderBuilder::default()
        .level(level)
        .proto(1)
        .predecessor(pred.hash.clone())
        .timestamp(10000000000 + (level as i64) * 30000 + additional_timestamp)
        .validation_pass(4)
        .operations_hash(
            "LLoZS2LW3rEi7KYU4ouBQtorua37aWWCtpDmv1n2x3xoKi6sVXLWp"
                .try_into()
                .unwrap(),
        )
        .fitness(vec![vec![0]])
        .context(
            "CoV8SQumiVU9saiu3FVNeDNewJaJH8yWdsGF3WLdsRr2P9S7MzCj"
                .try_into()
                .unwrap(),
        )
        .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
        .build()
        .unwrap();
    BlockHeaderWithHash::new(block_header).unwrap()
}

pub fn build_expected_history(
    chain: &Vec<BlockHeaderWithHash>,
    node_pkh: CryptoboxPublicKeyHash,
    peer_pkh: CryptoboxPublicKeyHash,
    current_head_level: Level,
) -> Vec<BlockHeaderWithHash> {
    let seed = Seed::new(&node_pkh, &peer_pkh);
    let mut step = Step::init(&seed, &chain.last().unwrap().hash);
    (0..200)
        .map(|_| step.next_step())
        .scan(current_head_level, |level, step| {
            *level -= step;
            Some(*level)
        })
        .take_while(|level| *level >= 0)
        .map(|level| chain[level as usize].clone())
        .collect()
}
