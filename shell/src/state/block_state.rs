// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::fmt;
use std::cmp;
use std::cmp::Ordering;
use std::sync::{Arc, RwLock};

use failure::_core::fmt::Formatter;
use rand::prelude::ThreadRng;
use rand::Rng;
use slog::Logger;

use crypto::hash::{BlockHash, ChainId};
use storage::{BlockHeaderWithHash, BlockMetaStorage, BlockStorage, BlockStorageReader, ChainMetaStorage, IteratorMode, StorageError};
use storage::block_meta_storage::Meta;
use storage::persistent::PersistentStorage;
use tezos_messages::Head;
use tezos_messages::p2p::encoding::block_header::{BlockHeader, Level};
use tezos_messages::p2p::encoding::current_branch::{CurrentBranchMessage, HISTORY_MAX_SIZE};

use crate::collections::{BlockData, UniqueBlockData};
use crate::shell_channel::{BlockApplied, CurrentMempoolState};
use crate::validation;

/// Holds state of all known blocks
pub struct BlockchainState {
    /// persistent block storage
    block_storage: BlockStorage,
    ///persistent block metadata storage
    block_meta_storage: BlockMetaStorage,
    ///persistent chain metadata storage
    chain_meta_storage: ChainMetaStorage,
    /// Current missing blocks.
    /// This represents a set of missing block we will try to retrieve in the future.
    /// Before we try to fetch missing block it is removed from this queue.
    /// Block is then sent to [`chain_manager`](crate::chain_manager::ChainManager) actor whose responsibility is to
    /// retrieve the block data. If the block data cannot be fetched it's the responsibility
    /// of the [`chain_manager`](crate::chain_manager::ChainManager) to return the block to this queue.
    missing_blocks: UniqueBlockData<MissingBlock>,
    chain_id: ChainId,
}

impl BlockchainState {
    pub fn new(persistent_storage: &PersistentStorage, chain_id: &ChainId) -> Self {
        BlockchainState {
            block_storage: BlockStorage::new(persistent_storage),
            block_meta_storage: BlockMetaStorage::new(persistent_storage),
            chain_meta_storage: ChainMetaStorage::new(persistent_storage),
            missing_blocks: UniqueBlockData::new(),
            chain_id: chain_id.clone(),
        }
    }

    /// Validate if we can accept branch
    pub fn can_accept_branch(&self, branch: &CurrentBranchMessage, current_head: &Option<Head>) -> bool {
        if branch.current_branch().current_head().level() <= 0 {
            return false;
        }
        true
    }

    /// Returns true, if [block] can be applied
    pub fn can_apply_block<'b, OP>(&self, (block, block_metadata): (&'b BlockHash, &'b Meta), operations_complete: OP) -> Result<bool, StorageError>
        where OP: Fn(&'b BlockHash) -> Result<bool, StorageError> /* func returns true, if operations are completed */
    {
        let block_predecessor = block_metadata.predecessor();

        // check if block is already applied, dont need to apply second time
        if block_metadata.is_applied() {
            return Ok(false);
        }

        // we need to have predecessor (every block has)
        if block_predecessor.is_none() {
            return Ok(false);
        }

        // if operations are not complete, we cannot apply block
        if !operations_complete(block)? {
            return Ok(false);
        }

        // check if predecesor is applied
        if let Some(predecessor) = block_predecessor {
            if let Some(predecessor_meta) = self.block_meta_storage.get(predecessor)? {
                return Ok(predecessor_meta.is_applied());
            }
        }

        Ok(false)
    }

    /// Resolves missing blocks and schedules them for download from network
    pub fn schedule_branch_bootstrap(&mut self, block_hash: &BlockHash, block_header: &BlockHeader, history: &Vec<BlockHash>) -> Result<(), StorageError> {
        let block_level = block_header.level();

        // at first schedule history - we try to prioritize download from the beginning, so the history is reversed here
        self.push_missing_history(
            history.iter().cloned().rev().collect(),
            block_level,
        )?;

        // schedule predecessor (if not present in history)
        if !history.contains(block_header.predecessor()) {
            self.push_missing_block(
                MissingBlock::with_level_guess(
                    block_header.predecessor().clone(),
                    std::cmp::max(1, block_level - 1),
                )
            )?;
        }

        // schedule also current_head
        self.push_missing_block(
            MissingBlock::with_level(
                block_hash.clone(),
                block_level,
            )
        )?;

        Ok(())
    }

    /// Resolve if new applied block can be set as new current head.
    /// Original algorithm is in [chain_validator][on_request], where just fitness is checked.
    /// Returns:
    /// - None, if head was not updated, means was ignored
    /// - Some(new_head, head_result)
    pub fn try_set_new_current_head(&self, potential_new_head: &BlockApplied, current_head: &Option<Head>, mempool_state: &Option<Arc<RwLock<CurrentMempoolState>>>) -> Result<Option<(Head, HeadResult)>, StorageError> {
        if let Some(current_head) = current_head {
            // get fitness from mempool, if not, than use current_head.fitness
            let current_context_fitness = match mempool_state {
                Some(state) => {
                    let mempool_state = state.read().unwrap();
                    match mempool_state.fitness.as_ref() {
                        Some(fitness) => fitness.clone(),
                        None => current_head.fitness().to_vec()
                    }
                }
                None => current_head.fitness().to_vec()
            };

            // need to check against current_head, if not accepted, just ignore potential head
            if !validation::can_accept_new_head(potential_new_head.header(), &current_head, &current_context_fitness) {
                // just ignore
                return Ok(None);
            }
        }

        // we need to check, if previous head is predecessor of new_head (for later use)
        let head_result = match &current_head {
            Some(previos_head) => if previos_head.hash().eq(potential_new_head.header().header.predecessor()) {
                HeadResult::HeadIncrement
            } else {
                // if previous head is not predecesor of new head, means it could be new branch
                HeadResult::BranchSwitch
            }
            None => HeadResult::HeadIncrement,
        };

        // this will be new head
        let head = Head::new(
            potential_new_head.header().hash.clone(),
            potential_new_head.header().header.level(),
            potential_new_head.header().header.fitness().clone(),
        );

        // set new head to db
        self.chain_meta_storage.set_current_head(&self.chain_id, head.clone())?;

        Ok(Some((head, head_result)))
    }

    pub fn process_block_header(&mut self, block_header: &BlockHeaderWithHash, log: &Logger) -> Result<(Meta, bool), StorageError> {
        // check if we already have seen predecessor
        self.push_missing_block(
            MissingBlock::with_level_guess(
                block_header.header.predecessor().clone(),
                block_header.header.level() - 1,
            )
        )?;

        // store block
        let is_new_block = self.block_storage.put_block_header(block_header)?;
        // update meta
        let metadata = self.block_meta_storage.put_block_header(block_header, &self.chain_id, &log)?;

        Ok((metadata, is_new_block))
    }

    #[inline]
    pub fn drain_missing_blocks(&mut self, n: usize, level_max: i32) -> Vec<MissingBlock> {
        (0..cmp::min(self.missing_blocks.len(), n))
            .filter_map(|_| {
                if self.missing_blocks.peek().filter(|block| block.fits_to_max(level_max)).is_some() {
                    self.missing_blocks.pop()
                } else {
                    None
                }
            })
            .collect()
    }

    #[inline]
    pub fn push_missing_block(&mut self, missing_block: MissingBlock) -> Result<(), StorageError> {
        if !self.block_storage.contains(&missing_block.block_hash)? {
            self.missing_blocks.push(missing_block);
        }
        Ok(())
    }

    #[inline]
    fn push_missing_history(&mut self, history: Vec<BlockHash>, level: Level) -> Result<(), StorageError> {
        let mut rng = rand::thread_rng();
        let history_max_parts = if history.len() < usize::from(HISTORY_MAX_SIZE) {
            history.len() as u8
        } else {
            HISTORY_MAX_SIZE
        };

        history.iter().enumerate()
            .map(|(idx, history_block_hash)| {
                self.push_missing_block(
                    MissingBlock::with_level_guess(
                        history_block_hash.clone(),
                        Self::guess_level(&mut rng, level, history_max_parts, idx),
                    )
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(())
    }

    #[inline]
    pub fn has_missing_blocks(&self) -> bool {
        !self.missing_blocks.is_empty()
    }

    #[inline]
    pub fn missing_blocks_count(&self) -> usize {
        self.missing_blocks.len()
    }

    pub fn hydrate(&mut self) -> Result<(), StorageError> {
        for (key, value) in self.block_meta_storage.iter(IteratorMode::Start)? {
            let (block_hash, meta) = (key?, value?);
            if meta.predecessor().is_none() && (meta.chain_id() == &self.chain_id) {
                self.missing_blocks.push(
                    MissingBlock::with_level(
                        block_hash,
                        meta.level(),
                    )
                );
            }
        }

        Ok(())
    }

    #[inline]
    pub fn get_chain_id(&self) -> &ChainId {
        &self.chain_id
    }

    pub fn get_history(&self) -> Result<Vec<BlockHash>, StorageError> {
        let history_max = 20;
        let mut history = Vec::with_capacity(history_max);
        let mut rng = rand::thread_rng();
        for (key, value) in self.block_meta_storage.iter(IteratorMode::Start)? {
            let pivot = (1 + rng.gen::<u8>() % 24) as i32;
            let (block_hash, meta) = (key?, value?);
            if meta.is_applied() && (meta.level() != 0) && (meta.level() % pivot == 0) && (meta.chain_id() == &self.chain_id) {
                history.push(block_hash);
                if history.len() >= history_max {
                    break;
                }
            }
        }
        Ok(history)
    }

    fn guess_level(rng: &mut ThreadRng, level: Level, parts: u8, index: usize) -> i32 {
        // e.g. we have: level 100 a 5 record in history, so split is 20, never <= 0
        let split = level / i32::from(parts);
        // corner case for 1 level;
        let split = cmp::max(1, split);

        // we try to guess level, because in history there is no level
        if index == 0 {
            // first block in history is always genesis
            0
        } else {
            // e.g. next block: idx * split, e.g. for index in history: 1 and split, we guess level is in range (0 * 20 - 1 * 20) -> (0, 20)
            let start_level = ((index as i32 - 1) * split) + 1;
            let end_level = (index as i32) * split;

            // corner case for 1 level
            let start_level = cmp::min(start_level, level);
            let end_level = cmp::min(end_level, level);

            if start_level == end_level {
                start_level
            } else {
                rng.gen_range(start_level, end_level)
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct MissingBlock {
    pub block_hash: BlockHash,
    // if level is known, we use level
    level: Option<i32>,
    // if level is unknow, we 'guess' level
    level_guess: Option<i32>,
}

impl BlockData for MissingBlock {
    #[inline]
    fn block_hash(&self) -> &BlockHash {
        &self.block_hash
    }
}

impl MissingBlock {
    pub fn with_level(block_hash: BlockHash, level: i32) -> Self {
        MissingBlock {
            block_hash,
            level: Some(level),
            level_guess: None,
        }
    }

    pub fn with_level_guess(block_hash: BlockHash, level_guess: i32) -> Self {
        MissingBlock {
            block_hash,
            level: None,
            level_guess: Some(level_guess),
        }
    }

    fn fits_to_max(&self, level_max: i32) -> bool {
        if let Some(level) = self.level {
            return level <= level_max;
        }

        if let Some(level_guess) = self.level_guess {
            return level_guess <= level_max;
        }

        // if both are None
        true
    }
}

impl PartialEq for MissingBlock {
    fn eq(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
    }
}

impl Eq for MissingBlock {}

impl PartialOrd for MissingBlock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MissingBlock {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_potential_level = match self.level {
            Some(level) => level,
            None => match self.level_guess {
                Some(level) => level,
                None => 0
            }
        };
        let other_potential_level = match other.level {
            Some(level) => level,
            None => match other.level_guess {
                Some(level) => level,
                None => 0
            }
        };

        // reverse, because we want lower level at begining
        self_potential_level.cmp(&other_potential_level).reverse()
    }
}

pub enum HeadResult {
    BranchSwitch,
    HeadIncrement,
}

impl fmt::Display for HeadResult {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match *self {
            HeadResult::BranchSwitch => write!(f, "BranchSwitch"),
            HeadResult::HeadIncrement => write!(f, "HeadIncrement"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_missing_blocks_has_correct_ordering() {
        let mut heap = UniqueBlockData::new();

        // simulate header and predecesor
        heap.push(MissingBlock::with_level(vec![0, 0, 0, 1], 10));
        heap.push(MissingBlock::with_level(vec![0, 0, 0, 2], 9));

        // simulate history
        heap.push(MissingBlock::with_level_guess(vec![0, 0, 0, 3], 4));
        heap.push(MissingBlock::with_level_guess(vec![0, 0, 0, 7], 0));
        heap.push(MissingBlock::with_level_guess(vec![0, 0, 0, 5], 2));
        heap.push(MissingBlock::with_level_guess(vec![0, 0, 0, 6], 1));
        heap.push(MissingBlock::with_level_guess(vec![0, 0, 0, 4], 3));

        // pop all from heap
        let ordered_hashes = (0..heap.len())
            .map(|_| heap.pop().unwrap())
            .map(|i| i.block_hash)
            .collect::<Vec<BlockHash>>();

        // from level: 0, 1, 2, 3, 4, 9, 10
        let expected_order = vec![
            vec![0, 0, 0, 7],
            vec![0, 0, 0, 6],
            vec![0, 0, 0, 5],
            vec![0, 0, 0, 4],
            vec![0, 0, 0, 3],
            vec![0, 0, 0, 2],
            vec![0, 0, 0, 1],
        ];

        assert_eq!(expected_order, ordered_hashes)
    }

    #[test]
    fn test_guess_level() {
        let mut rng = rand::thread_rng();

        // for block 0 in history (always 0)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 100, 5, 0);
            assert_eq!(level, 0);
        }

        // for block 1 in history [1, 20)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 100, 5, 1);
            assert!(level >= 1 && level < 20);
        }

        // for block 2 in history [20, 40)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 100, 5, 2);
            assert!(level >= 20 && level < 40);
        }

        // for block 3 in history [40, 60)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 100, 5, 3);
            assert!(level >= 40 && level < 60);
        }

        // for block 4 in history [60, 80)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 100, 5, 4);
            assert!(level >= 60 && level < 80);
        }

        // for block 5 in history [80, 100)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 100, 5, 5);
            assert!(level >= 80 && level < 100);
        }

        // for block 0 in history [0, 1)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 1, 1, 0);
            assert!(level >= 0 && level < 1);
        }

        // corner case (for level 1 if there are two elements)
        // for block 0 in history [0, 1)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 1, 2, 0);
            assert!(level >= 0 && level < 1);
        }

        // corner case (for level 1 if there are two elements)
        // for block 1 in history [1, 2)
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 1, 2, 1);
            assert!(level >= 1 && level < 2);
        }

        // corner cases
        for _ in 0..100 {
            let level = BlockchainState::guess_level(&mut rng, 1, 3, 0);
            assert!(level >= 0 && level < 1);
            let level = BlockchainState::guess_level(&mut rng, 1, 3, 1);
            assert!(level >= 1 && level < 2);
            let level = BlockchainState::guess_level(&mut rng, 1, 3, 2);
            assert!(level >= 1 && level < 2);
        }
    }
}