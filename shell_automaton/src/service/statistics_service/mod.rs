// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::hash_map;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use tezos_messages::p2p::encoding::block_header::Level;

pub type BlocksStats = HashMap<Arc<BlockHash>, BlockStats>;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct BlockStats {
    pub level: Option<Level>,

    pub load_data_start: Option<u64>,
    pub load_data_end: Option<u64>,

    pub apply_block_start: Option<u64>,
    pub apply_block_end: Option<u64>,

    pub store_result_start: Option<u64>,
    pub store_result_end: Option<u64>,
}

#[derive(Debug, Default)]
pub struct StatisticsService {
    blocks: BlocksStats,
    blocks_by_level: VecDeque<Arc<BlockHash>>,
}

impl StatisticsService {
    pub fn stats_get(&self) -> &BlocksStats {
        &self.blocks
    }

    pub fn block_new(&mut self, block_hash: Arc<BlockHash>) {
        if let hash_map::Entry::Vacant(e) = self.blocks.entry(block_hash) {
            let block_hash = e.key().clone();
            e.insert(Default::default());
            self.blocks_by_level.push_back(block_hash);

            if self.blocks_by_level.len() > 2000 {
                if let Some(oldest_block) = self.blocks_by_level.pop_front() {
                    self.blocks.remove(&oldest_block);
                }
            }
        }
    }

    /// Started loading block data from storage for block application.
    pub fn block_load_data_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks
            .get_mut(block_hash)
            .map(|v| v.load_data_start = Some(time));
    }

    /// Finished loading block data from storage for block application.
    pub fn block_load_data_end(&mut self, block_hash: &BlockHash, block_level: Level, time: u64) {
        self.blocks.get_mut(block_hash).map(|v| {
            v.level = Some(block_level);
            v.load_data_end = Some(time);
        });
    }

    pub fn block_apply_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks
            .get_mut(block_hash)
            .map(|v| v.apply_block_start = Some(time));
    }

    pub fn block_apply_end(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks
            .get_mut(block_hash)
            .map(|v| v.apply_block_end = Some(time));
    }

    pub fn block_store_result_start(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks
            .get_mut(block_hash)
            .map(|v| v.store_result_start = Some(time));
    }

    pub fn block_store_result_end(&mut self, block_hash: &BlockHash, time: u64) {
        self.blocks
            .get_mut(block_hash)
            .map(|v| v.store_result_end = Some(time));
    }
}
