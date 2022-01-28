// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::hash_map;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crypto::hash::BlockHash;
use tezos_api::ffi::{ApplyBlockExecutionTimestamps, ApplyBlockResponse};
use tezos_messages::p2p::encoding::block_header::Level;

fn ocaml_time_normalize(ocaml_time: f64) -> u64 {
    (ocaml_time * 1_000_000_000.0) as u64
}

pub type BlocksStats = HashMap<Arc<BlockHash>, BlockStats>;

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct BlockStats {
    pub level: Option<Level>,

    pub load_data_start: Option<u64>,
    pub load_data_end: Option<u64>,

    pub apply_block_start: Option<u64>,
    pub apply_block_end: Option<u64>,
    pub apply_block_stats: Option<ApplyBlockProtocolStats>,

    pub store_result_start: Option<u64>,
    pub store_result_end: Option<u64>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone)]
pub struct ApplyBlockProtocolStats {
    pub apply_start: u64,
    pub operations_decoding_start: u64,
    pub operations_decoding_end: u64,
    pub operations_application: Vec<Vec<(u64, u64)>>,
    pub operations_metadata_encoding_start: u64,
    pub operations_metadata_encoding_end: u64,
    pub begin_application_start: u64,
    pub begin_application_end: u64,
    pub finalize_block_start: u64,
    pub finalize_block_end: u64,
    pub collect_new_rolls_owner_snapshots_start: u64,
    pub collect_new_rolls_owner_snapshots_end: u64,
    pub commit_start: u64,
    pub commit_end: u64,
    pub apply_end: u64,
}

impl From<&ApplyBlockExecutionTimestamps> for ApplyBlockProtocolStats {
    fn from(v: &ApplyBlockExecutionTimestamps) -> Self {
        Self {
            apply_start: ocaml_time_normalize(v.apply_start_t),
            operations_decoding_start: ocaml_time_normalize(v.operations_decoding_start_t),
            operations_decoding_end: ocaml_time_normalize(v.operations_decoding_end_t),
            operations_application: v
                .operations_application_timestamps
                .iter()
                .map(|l| {
                    l.iter()
                        .map(|(start, end)| {
                            (ocaml_time_normalize(*start), ocaml_time_normalize(*end))
                        })
                        .collect()
                })
                .collect(),
            operations_metadata_encoding_start: ocaml_time_normalize(
                v.operations_metadata_encoding_start_t,
            ),
            operations_metadata_encoding_end: ocaml_time_normalize(
                v.operations_metadata_encoding_end_t,
            ),
            begin_application_start: ocaml_time_normalize(v.begin_application_start_t),
            begin_application_end: ocaml_time_normalize(v.begin_application_end_t),
            finalize_block_start: ocaml_time_normalize(v.finalize_block_start_t),
            finalize_block_end: ocaml_time_normalize(v.finalize_block_end_t),
            collect_new_rolls_owner_snapshots_start: ocaml_time_normalize(
                v.collect_new_rolls_owner_snapshots_start_t,
            ),
            collect_new_rolls_owner_snapshots_end: ocaml_time_normalize(
                v.collect_new_rolls_owner_snapshots_end_t,
            ),
            commit_start: ocaml_time_normalize(v.commit_start_t),
            commit_end: ocaml_time_normalize(v.commit_end_t),
            apply_end: ocaml_time_normalize(v.apply_end_t),
        }
    }
}

#[derive(Debug, Default)]
pub struct StatisticsService {
    blocks: BlocksStats,
    blocks_by_level: VecDeque<Arc<BlockHash>>,
}

impl StatisticsService {
    pub fn block_stats_get_all(&self) -> &BlocksStats {
        &self.blocks
    }

    fn find_min_index_for_block_level(&self, level: Level) -> Option<usize> {
        let first_block_level = self
            .blocks_by_level
            .get(0)
            .and_then(|hash| self.blocks.get(hash))
            .and_then(|block| block.level)?;

        let index = level.checked_sub(first_block_level)?;
        if index < 0 {
            return None;
        }

        Some(index as usize)
    }

    fn blocks_for_level_iter<'a>(
        &'a self,
        level: Level,
    ) -> impl 'a + Iterator<Item = &'a BlockStats> {
        let start_index = self
            .find_min_index_for_block_level(level)
            .unwrap_or(usize::MAX);

        self.blocks_by_level
            .iter()
            .skip(start_index)
            .filter_map(move |hash| self.blocks.get(hash))
            .filter(move |v| v.level.filter(|l| *l == level).is_some())
    }

    pub fn block_stats_get_by_level(&self, level: Level) -> Option<&BlockStats> {
        self.blocks_for_level_iter(level).nth(0)
    }

    pub fn block_new(&mut self, block_hash: Arc<BlockHash>) {
        if let hash_map::Entry::Vacant(e) = self.blocks.entry(block_hash) {
            let block_hash = e.key().clone();
            e.insert(Default::default());
            self.blocks_by_level.push_back(block_hash);

            if self.blocks_by_level.len() > 20000 {
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

    pub fn block_apply_end(
        &mut self,
        block_hash: &BlockHash,
        time: u64,
        result: &ApplyBlockResponse,
    ) {
        self.blocks.get_mut(block_hash).map(|v| {
            v.apply_block_stats = Some((&result.execution_timestamps).into());
            v.apply_block_end = Some(time)
        });
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
