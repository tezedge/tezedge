// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Temporary bootstrap state which helps to download and validate branches from peers
//!
//! - every peer has his own BootstrapState
//! - bootstrap state is initialized from branch history, which is splitted to partitions
//!
//! - it is king of bingo, where we prepare block intervals, and we check/mark what is downloaded/applied, and what needs to be downloaded or applied

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crypto::hash::{BlockHash, ChainId};
use tezos_messages::p2p::encoding::block_header::Level;

use crate::state::{BlockApplyBatch, StateError};

/// BootstrapState helps to easily manage/mutate inner state
pub struct BootstrapState {
    /// We can identify stalled pipelines by this atribute and take action like disconnect peer
    last_updated: Instant,

    /// Level of the highest block from all intervals
    to_level: Arc<Level>,
    chain_id: Arc<ChainId>,

    /// Partitions are expected to be ordered from the lowest_level/oldest block
    intervals: Vec<BootstrapInterval>,
}

impl BootstrapState {
    /// Creates new pipeline, it must always start with applied block (at least with genesis),
    /// This block is used to define start of the branch
    pub fn new(
        chain_id: Arc<ChainId>,
        first_applied_block: Arc<BlockHash>,
        blocks: Vec<Arc<BlockHash>>,
        to_level: Arc<Level>,
    ) -> BootstrapState {
        BootstrapState {
            chain_id,
            last_updated: Instant::now(),
            intervals: BootstrapInterval::split(first_applied_block, blocks),
            to_level,
        }
    }

    pub fn chain_id(&self) -> &Arc<ChainId> {
        &self.chain_id
    }

    pub fn to_level(&self) -> &Arc<Level> {
        &self.to_level
    }

    /// This finds block, which should be downloaded first and refreshes state for all touched blocks
    ///
    /// max_interval_ahead_count - we want/need the oldest header at first, so we dont download agressively the whole chain from all intervals (which could speedup download),
    ///                            so we will download just few intervals ahead
    ///
    /// BM - callback then returns actual metadata state + his predecessor
    pub fn find_next_blocks_to_download<BM>(
        &mut self,
        requested_count: usize,
        mut ignored_blocks: HashSet<Arc<BlockHash>>,
        max_interval_ahead_count: i8,
        get_block_metadata: BM,
    ) -> Result<Vec<Arc<BlockHash>>, StateError>
    where
        BM: Fn(&BlockHash) -> Result<Option<(InnerBlockState, Arc<BlockHash>)>, StateError>,
    {
        let mut result = Vec::with_capacity(requested_count);
        if requested_count == 0 {
            return Ok(result);
        }
        let mut max_interval_ahead_count = max_interval_ahead_count;

        // lets iterate intervals
        for interval in self.intervals.iter_mut() {
            // if interval is downloaded, just skip it
            if interval.all_blocks_downloaded {
                continue;
            }

            // let walk throught interval and resolve first missing block
            let (missing_block, was_interval_updated) =
                interval.find_first_missing_block(&ignored_blocks, &get_block_metadata)?;

            // add to scheduled and continue
            if let Some(missing_block) = missing_block {
                result.push(missing_block.clone());
                ignored_blocks.insert(missing_block);
                max_interval_ahead_count -= 1;
            }
            if was_interval_updated {
                self.last_updated = Instant::now();
            }

            if result.len() >= requested_count || max_interval_ahead_count <= 0 {
                return Ok(result);
            }
        }

        Ok(result)
    }

    /// This finds block, which we miss operations and should be downloaded first and also refreshes state for all touched blocks
    ///
    /// max_interval_ahead_count - we want/need the oldest header at first, so we dont download agressively the whole chain from all intervals (which could speedup download),
    ///                            so we will download just few intervals ahead
    ///
    /// OC - callback then returns if operations are already completelly downloaded
    pub fn find_next_block_operations_to_download<OC>(
        &mut self,
        requested_count: usize,
        mut ignored_blocks: HashSet<Arc<BlockHash>>,
        max_interval_ahead_count: i8,
        is_operations_complete: OC,
    ) -> Result<Vec<Arc<BlockHash>>, StateError>
    where
        OC: Fn(&BlockHash) -> Result<bool, StateError>,
    {
        let mut result = Vec::with_capacity(requested_count);
        if requested_count == 0 {
            return Ok(result);
        }

        let mut max_interval_ahead_count = max_interval_ahead_count;

        // lets iterate intervals
        for interval in self.intervals.iter_mut() {
            // if interval is downloaded, just skip it
            if interval.all_operations_downloaded {
                continue;
            }

            let partial_requested_count = if result.len() >= requested_count {
                return Ok(result);
            } else {
                requested_count - result.len()
            };

            // let walk throught interval and resolve missing block from this interval
            let (missing_blocks, was_interval_updated) = interval
                .find_block_with_missing_operations(
                    partial_requested_count,
                    &ignored_blocks,
                    &is_operations_complete,
                )?;

            if !missing_blocks.is_empty() {
                // add to scheduled and continue
                result.extend(missing_blocks.clone());
                ignored_blocks.extend(missing_blocks);
                max_interval_ahead_count -= 1;
            }
            if was_interval_updated {
                self.last_updated = Instant::now();
            }

            if result.len() >= requested_count || max_interval_ahead_count <= 0 {
                return Ok(result);
            }
        }

        Ok(result)
    }

    /// This finds block, for which we should apply and also refreshes state for all touched blocks.
    ///
    /// IA - callback then returns if block is already aplpied
    pub fn find_next_block_to_apply<IA>(
        &mut self,
        max_block_apply_batch: usize,
        is_block_applied: IA,
    ) -> Result<Option<BlockApplyBatch>, StateError>
    where
        IA: Fn(&BlockHash) -> Result<bool, StateError>,
    {
        let mut last_marked_as_applied_block = None;
        let mut batch_for_apply: Option<BlockApplyBatch> = None;
        let mut previous_block: Option<(Arc<BlockHash>, bool)> = None;

        for interval in self.intervals.iter_mut() {
            if interval.all_block_applied {
                continue;
            }

            let mut interval_break = false;

            // check all blocks in interval
            // get first non-applied block
            for b in interval.blocks.iter_mut() {
                // skip applied
                if b.applied {
                    // continue to check next block
                    previous_block = Some((b.block_hash.clone(), b.applied));
                    continue;
                }

                // if previous is the same as a block - can happen on the border of interevals
                // where last block of previous interval is the first block of next interval
                if let Some((previous_block_hash, _)) = previous_block.as_ref() {
                    if b.block_hash.as_ref().eq(previous_block_hash.as_ref()) {
                        // just continue, previous_block is the same and we processed it right before
                        continue;
                    }
                }

                // check if block is already applied,
                // we check this only if did not find batch start yet - optimization to prevent unnecesesery calls to is_block_applied
                if batch_for_apply.is_none() && is_block_applied(&b.block_hash)? {
                    // refresh state for block
                    if b.update(&InnerBlockState {
                        applied: true,
                        block_downloaded: true,
                        operations_downloaded: true,
                    }) {
                        last_marked_as_applied_block = Some(b.block_hash.clone());
                        self.last_updated = Instant::now();
                    }

                    // continue to check next block
                    previous_block = Some((b.block_hash.clone(), b.applied));
                    continue;
                }

                // if block and operations are downloaded, we check his predecessor, if applied, we can apply
                if b.block_downloaded && b.operations_downloaded {
                    // predecessor must match - continuos chain of blocks
                    if let Some(block_predecessor) = b.predecessor_block_hash.as_ref() {
                        if let Some((previous_block_hash, previous_is_applied)) =
                            previous_block.as_ref()
                        {
                            if block_predecessor.as_ref().eq(previous_block_hash.as_ref()) {
                                // if we came here, we have still continuous chain

                                // if previos block is applied and b is not, then we have batch start candidate
                                if *previous_is_applied {
                                    // start batch and continue to next blocks
                                    batch_for_apply =
                                        Some(BlockApplyBatch::start_batch(b.block_hash.clone()));

                                    if max_block_apply_batch > 0 {
                                        // continue to check next block
                                        previous_block = Some((b.block_hash.clone(), b.applied));
                                        continue;
                                    }
                                } else if let Some(batch) = batch_for_apply.as_mut() {
                                    // if previous block is not applied, means we can add it to batch
                                    batch.add_successor(b.block_hash.clone());
                                    if batch.successors_size() < max_block_apply_batch {
                                        // continue to check next block
                                        previous_block = Some((b.block_hash.clone(), b.applied));
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }

                // if we did not find anything in this interval and this interval is not whole applied, there is no reason to continue to next interval
                interval_break = true;
                break;
            }

            // if we stop interval with break, we dont want to continue to next interval
            if interval_break {
                break;
            }
        }

        // handle last block, which we marked as applied - to remove the rest of the intervals to speedup
        if let Some(last_marked_as_applied_block) = last_marked_as_applied_block {
            self.block_applied(&last_marked_as_applied_block);
        }

        Ok(batch_for_apply)
    }

    /// Notify state requested_block_hash was downloaded
    ///
    /// <BM> callback which return (bool as downloaded, bool as applied, bool as are_operations_complete)
    pub fn block_downloaded(
        &mut self,
        requested_block_hash: &BlockHash,
        requested_block_new_inner_state: &InnerBlockState,
        predecessor_block_hash: Arc<BlockHash>,
    ) {
        // find first interval with this block
        let interval_to_handle = self.intervals.iter_mut().enumerate().find(|(_, interval)| {
            // find first interval with this block
            interval
                .blocks
                .iter()
                .filter(|b| !b.applied)
                .any(|b| b.block_hash.as_ref().eq(requested_block_hash))
        });

        // Note:end_interval
        let mut end_of_interval: Option<(usize, InnerBlockState)> = None;

        if let Some((interaval_idx, interval)) = interval_to_handle {
            let block_count = interval.blocks.len();
            let mut predecessor_insert_to_index: Option<usize> = None;

            // update metadata
            for (block_index, b) in interval.blocks.iter_mut().enumerate() {
                // update request block as downloaded
                if requested_block_hash.eq(b.block_hash.as_ref()) {
                    // if applied, then skip
                    if b.applied {
                        continue;
                    }

                    // update its state
                    if b.update(&requested_block_new_inner_state) {
                        self.last_updated = Instant::now();
                    }
                    if b.update_predecessor(predecessor_block_hash.clone()) {
                        self.last_updated = Instant::now();
                    }

                    predecessor_insert_to_index = Some(block_index);

                    // Note:end_interval: handle end of the interval
                    if (block_count - 1) == block_index {
                        end_of_interval = Some((
                            interaval_idx,
                            InnerBlockState {
                                block_downloaded: b.block_downloaded,
                                operations_downloaded: b.operations_downloaded,
                                applied: b.applied,
                            },
                        ));
                    }

                    break;
                }
            }

            if let Some(predecessor_insert_to_index) = predecessor_insert_to_index {
                // we cannot add predecessor before begining of interval
                if predecessor_insert_to_index > 0 {
                    // check if predecessor block is really our predecessor
                    if let Some(potential_predecessor) =
                        interval.blocks.get(predecessor_insert_to_index - 1)
                    {
                        // if not, we need to insert predecessor here
                        if !potential_predecessor
                            .block_hash
                            .as_ref()
                            .eq(&predecessor_block_hash)
                        {
                            interval.blocks.insert(
                                predecessor_insert_to_index,
                                BlockState::new(predecessor_block_hash),
                            );
                        }
                    }
                }
            }

            // check if have downloaded whole interval
            if interval.check_all_blocks_downloaded() {
                self.last_updated = Instant::now();
            }
        };

        // Note:end_interval we need to copy this to beginging of the next interval
        if let Some((interval_idx, end_of_previos_interval_state)) = end_of_interval {
            if let Some(next_interval) = self.intervals.get_mut(interval_idx + 1) {
                // get first block
                if let Some(begin) = next_interval.blocks.get_mut(0) {
                    if begin.update(&end_of_previos_interval_state) {
                        self.last_updated = Instant::now();
                    }
                }

                // check if have downloaded whole next interval
                if next_interval.check_all_blocks_downloaded() {
                    self.last_updated = Instant::now();
                }
            }
        }

        // check, what if requested block is already applied (this removes all predecessor and/or the whole interval)
        if requested_block_new_inner_state.applied {
            self.block_applied(&requested_block_hash);
        }
    }

    /// Notify state requested_block_hash has all downloaded operations
    ///
    /// <BM> callback wchic return (bool as downloaded, bool as applied, bool as are_operations_complete)
    pub fn block_operations_downloaded(&mut self, requested_block_hash: &BlockHash) {
        // find first interval with this block
        let interval_to_handle = self.intervals.iter_mut().enumerate().find(|(_, interval)| {
            // find first interval with this block
            interval
                .blocks
                .iter()
                .filter(|b| !b.applied)
                .any(|b| b.block_hash.as_ref().eq(requested_block_hash))
        });

        // Note:end_interval
        let mut end_of_interval: Option<(usize, InnerBlockState)> = None;

        if let Some((interaval_idx, interval)) = interval_to_handle {
            // find block and update metadata
            let block_count = interval.blocks.len();
            for (block_index, mut b) in interval.blocks.iter_mut().enumerate() {
                // update request block as downloaded
                if requested_block_hash.eq(b.block_hash.as_ref()) {
                    // now we can update
                    if !b.operations_downloaded {
                        b.operations_downloaded = true;
                        self.last_updated = Instant::now();
                    }

                    // Note:end_interval: handle end of the interval
                    if (block_count - 1) == block_index {
                        end_of_interval = Some((
                            interaval_idx,
                            InnerBlockState {
                                block_downloaded: b.block_downloaded,
                                operations_downloaded: b.operations_downloaded,
                                applied: b.applied,
                            },
                        ));
                    }

                    break;
                }
            }
        };

        // Note:end_interval we need to copy this to beginging of the next interval
        if let Some((interval_idx, end_of_previos_interval_state)) = end_of_interval {
            if let Some(next_interval) = self.intervals.get_mut(interval_idx + 1) {
                // get first block
                if let Some(begin) = next_interval.blocks.get_mut(0) {
                    if begin.update(&end_of_previos_interval_state) {
                        self.last_updated = Instant::now();
                    }
                }
            }
        }
    }

    /// Notify state requested_block_hash has been applied
    ///
    /// <BM> callback wchic return (bool as downloaded, bool as applied, bool as are_operations_complete)
    pub fn block_applied(&mut self, requested_block_hash: &BlockHash) {
        // find first interval with this block
        let interval_to_handle = self.intervals.iter_mut().enumerate().find(|(_, interval)| {
            // find first interval with this block
            interval
                .blocks
                .iter()
                .any(|b| b.block_hash.as_ref().eq(requested_block_hash))
        });

        if let Some((interval_idx, interval)) = interval_to_handle {
            // Note:end_interval
            let mut end_of_interval: Option<(usize, InnerBlockState)> = None;

            // find block and update metadata
            let block_count = interval.blocks.len();
            let mut block_index_to_remove_before = None;
            for (block_index, mut b) in interval.blocks.iter_mut().enumerate() {
                // update request block as downloaded
                if requested_block_hash.eq(b.block_hash.as_ref()) {
                    // now we can update
                    if !b.applied {
                        b.applied = true;
                        self.last_updated = Instant::now();
                    }
                    if b.applied {
                        block_index_to_remove_before = Some(block_index);
                    }

                    // Note:end_interval: handle end of the interval
                    if (block_count - 1) == block_index {
                        end_of_interval = Some((
                            interval_idx,
                            InnerBlockState {
                                block_downloaded: b.block_downloaded,
                                operations_downloaded: b.operations_downloaded,
                                applied: b.applied,
                            },
                        ));
                    }

                    break;
                }
            }

            if let Some(remove_index) = block_index_to_remove_before {
                // we dont want to remove the block, just the blocks before, because at least first block must be applied
                for _ in 0..remove_index {
                    let _ = interval.blocks.remove(0);
                    self.last_updated = Instant::now();
                }
                if interval.check_all_blocks_downloaded() {
                    self.last_updated = Instant::now();
                }
            }

            // Note:end_interval we need to copy this to beginging of the next interval
            if let Some((interval_idx, end_of_previos_interval_state)) = end_of_interval {
                if let Some(next_interval) = self.intervals.get_mut(interval_idx + 1) {
                    // get first block
                    if let Some(begin) = next_interval.blocks.get_mut(0) {
                        if begin.update(&end_of_previos_interval_state) {
                            self.last_updated = Instant::now();
                        }
                    }
                }
            }

            // remove interval if it is empty
            if let Some(interval_to_remove) = self.intervals.get_mut(interval_idx) {
                if interval_to_remove.blocks.len() <= 1 {
                    let all_applied = interval_to_remove.blocks.iter().all(|b| b.applied);
                    if all_applied {
                        let _ = self.intervals.remove(interval_idx);
                        self.last_updated = Instant::now();
                    }
                }

                // here we need to remove all previous interval, becuse we dont need them, when higher block was applied
                if interval_idx > 0 {
                    // so, remove all previous
                    for _ in 0..interval_idx {
                        let _ = self.intervals.remove(0);
                        self.last_updated = Instant::now();
                    }
                }
            }
        };
    }

    pub fn is_stalled(&self, stale_timeout: Duration) -> bool {
        self.last_updated.elapsed() > stale_timeout
    }

    pub fn is_done(&self) -> bool {
        self.intervals.is_empty()
    }
}

/// Partions represents sequence from history, ordered from lowest_level/oldest block
///
/// History:
///     (bh1, bh2, bh3, bh4, bh5)
/// Is splitted to:
///     (bh1, bh2)
///     (bh2, bh3)
///     (bh3, bh4)
///     (bh4, bh5)
struct BootstrapInterval {
    all_blocks_downloaded: bool,
    all_operations_downloaded: bool,
    all_block_applied: bool,

    // TODO: we should maybe add here: (start: Arc<Mutex<BlockState>>, end: Arc<Mutex<BlockState>>)
    //       it would simplify maybe handling see: Note:begining_interval, Note:end_interval
    blocks: Vec<BlockState>,
}

impl BootstrapInterval {
    fn new(block_hash: Arc<BlockHash>) -> Self {
        Self {
            all_blocks_downloaded: false,
            all_operations_downloaded: false,
            all_block_applied: false,
            blocks: vec![BlockState::new(block_hash)],
        }
    }

    fn new_applied(block_hash: Arc<BlockHash>) -> Self {
        Self {
            all_blocks_downloaded: false,
            all_operations_downloaded: false,
            all_block_applied: false,
            blocks: vec![BlockState::new_applied(block_hash)],
        }
    }

    fn new_with_left(left: BlockState, right_block_hash: Arc<BlockHash>) -> Self {
        Self {
            all_blocks_downloaded: false,
            all_operations_downloaded: false,
            all_block_applied: false,
            blocks: vec![left, BlockState::new(right_block_hash)],
        }
    }

    fn split(
        first_applied_block: Arc<BlockHash>,
        blocks: Vec<Arc<BlockHash>>,
    ) -> Vec<BootstrapInterval> {
        let mut intervals: Vec<BootstrapInterval> = Vec::with_capacity(blocks.len() / 2);

        // insert first interval with applied block
        intervals.push(BootstrapInterval::new_applied(first_applied_block));

        // now split to interval the rest of the blocks
        for bh in blocks {
            let new_interval = match intervals.last_mut() {
                Some(last_part) => {
                    if last_part.blocks.len() >= 2 {
                        // previous is full, so add new one
                        if let Some(last_block) = last_part.blocks.last() {
                            Some(BootstrapInterval::new_with_left(last_block.clone(), bh))
                        } else {
                            // this cannot happen
                            last_part.blocks.push(BlockState::new(bh));
                            None
                        }
                    } else {
                        // close interval
                        last_part.blocks.push(BlockState::new(bh));
                        None
                    }
                }
                None => Some(BootstrapInterval::new(bh)),
            };

            if let Some(new_interval) = new_interval {
                intervals.push(new_interval);
            }
        }

        intervals
    }

    /// Walks trought interval blocks and checks if we can feed it,
    /// this also can modify/mark all other blocks with get_block_metadata,
    /// so it can modify this interval
    ///
    /// BM - callback then returns actual metadata state + his predecessor
    ///
    /// Returns tuple:
    ///     0 -> missing block, which we want to download
    ///     1 -> true/false if we updated interval
    fn find_first_missing_block<BM>(
        &mut self,
        ignored_blocks: &HashSet<Arc<BlockHash>>,
        get_block_metadata: BM,
    ) -> Result<(Option<Arc<BlockHash>>, bool), StateError>
    where
        BM: Fn(&BlockHash) -> Result<Option<(InnerBlockState, Arc<BlockHash>)>, StateError>,
    {
        let mut stop_block: Option<Arc<BlockHash>> = None;
        let mut was_updated = false;
        let mut start_from_the_begining = true;
        while start_from_the_begining {
            start_from_the_begining = false;

            let mut insert_predecessor: Option<(usize, Arc<BlockHash>)> = None;
            let mut previous: Option<Arc<BlockHash>> = None;

            for (idx, b) in self.blocks.iter_mut().enumerate() {
                // stop block optimization, because we are just prepending to blocks (optimization)
                if let Some(stop_block) = stop_block.as_ref() {
                    if stop_block.eq(&b.block_hash) {
                        break;
                    }
                }

                if !b.block_downloaded {
                    match get_block_metadata(&b.block_hash)? {
                        Some((new_inner_state, predecessor)) => {
                            // lets update state for blocks
                            if new_inner_state.block_downloaded {
                                if b.update(&new_inner_state) {
                                    was_updated = true;
                                }
                                if b.update_predecessor(predecessor) {
                                    was_updated = true;
                                }
                            } else {
                                // still not downloaded, so we schedule and continue
                                if !ignored_blocks.contains(&b.block_hash) {
                                    return Ok((Some(b.block_hash.clone()), was_updated));
                                }
                            }
                        }
                        None => {
                            // no metadata for block, so we schedule and continue
                            if !ignored_blocks.contains(&b.block_hash) {
                                return Ok((Some(b.block_hash.clone()), was_updated));
                            }
                        }
                    }
                }

                // check if we missing predecessor in the interval (skipping check for the first block)
                if let Some(previous_block) = previous.as_ref() {
                    match b.predecessor_block_hash.as_ref() {
                        Some(block_predecessor) => {
                            if !previous_block.as_ref().eq(&block_predecessor) {
                                // if previos block is not our predecessor, we need to insert it there
                                insert_predecessor = Some((idx, block_predecessor.clone()));
                                stop_block = Some(b.block_hash.clone());
                                break;
                            }
                        }
                        None => {
                            // strange, we dont know predecessor of block, so we schedule block downloading once more
                            if !ignored_blocks.contains(&b.block_hash) {
                                return Ok((Some(b.block_hash.clone()), was_updated));
                            }
                        }
                    }
                }

                previous = Some(b.block_hash.clone());
            }

            // handle missing predecessor
            if let Some((predecessor_idx, predecessor_block_hash)) = insert_predecessor {
                // create actual state for predecessor
                let predecessor_state = if let Some((new_state, his_predecessor)) =
                    get_block_metadata(&predecessor_block_hash)?
                {
                    let mut predecessor_state = BlockState::new(predecessor_block_hash);
                    let _ = predecessor_state.update(&new_state);
                    let _ = predecessor_state.update_predecessor(his_predecessor);
                    predecessor_state
                } else {
                    BlockState::new(predecessor_block_hash)
                };

                // insert predecessor (we cannot insert predecessor before beginging of interval)
                if predecessor_idx != 0 {
                    self.blocks.insert(predecessor_idx, predecessor_state);
                }
                was_updated = true;
                start_from_the_begining = true;
            }
        }

        if self.check_all_blocks_downloaded() {
            was_updated = true;
        }

        Ok((None, was_updated))
    }

    /// Check, if we had downloaded the whole interval,
    /// if flag ['all_blocks_downloaded'] was set, then returns true.
    fn check_all_blocks_downloaded(&mut self) -> bool {
        if self.all_blocks_downloaded {
            return false;
        }

        // if we came here, we just need to check if we downloaded the whole interval
        let mut previous: Option<&Arc<BlockHash>> = None;
        let mut all_blocks_downloaded = true;

        for b in &self.blocks {
            if !b.block_downloaded {
                all_blocks_downloaded = false;
                break;
            }
            if let Some(previous) = previous {
                if let Some(block_predecessor) = &b.predecessor_block_hash {
                    if !block_predecessor.eq(&previous) {
                        all_blocks_downloaded = false;
                        break;
                    }
                } else {
                    // missing predecessor
                    all_blocks_downloaded = false;
                    break;
                }
            }
            previous = Some(&b.block_hash);
        }

        if all_blocks_downloaded {
            self.all_blocks_downloaded = true;
            self.all_operations_downloaded = self.blocks.iter().all(|b| b.operations_downloaded);
            true
        } else {
            false
        }
    }

    pub(crate) fn find_block_with_missing_operations<OC>(
        &mut self,
        requested_count: usize,
        ignored_blocks: &HashSet<Arc<BlockHash>>,
        is_operations_complete: OC,
    ) -> Result<(Vec<Arc<BlockHash>>, bool), StateError>
    where
        OC: Fn(&BlockHash) -> Result<bool, StateError>,
    {
        if self.all_operations_downloaded {
            return Ok((vec![], false));
        }

        let mut result = Vec::with_capacity(requested_count);
        let mut was_updated = false;

        for b in self.blocks.iter_mut() {
            // if downloaded, just skip
            if b.operations_downloaded {
                continue;
            }

            // ignored skip also
            if ignored_blocks.contains(&b.block_hash) {
                continue;
            }

            // check result
            if result.len() >= requested_count {
                break;
            }

            // check real status
            if is_operations_complete(&b.block_hash)? {
                b.operations_downloaded = true;
                was_updated = true;
            } else {
                result.push(b.block_hash.clone());
            }
        }

        // check interval has all operations downloaded
        if self.all_blocks_downloaded {
            self.all_operations_downloaded = self.blocks.iter().all(|b| b.operations_downloaded);
        }

        Ok((result, was_updated))
    }
}

#[derive(Clone, Debug)]
pub struct InnerBlockState {
    pub block_downloaded: bool,
    pub operations_downloaded: bool,
    pub applied: bool,
}

#[derive(Clone)]
struct BlockState {
    block_hash: Arc<BlockHash>,
    predecessor_block_hash: Option<Arc<BlockHash>>,

    applied: bool,
    block_downloaded: bool,
    operations_downloaded: bool,
}

impl BlockState {
    fn new(block_hash: Arc<BlockHash>) -> Self {
        BlockState {
            block_hash,
            predecessor_block_hash: None,
            block_downloaded: false,
            operations_downloaded: false,
            applied: false,
        }
    }

    fn new_applied(block_hash: Arc<BlockHash>) -> Self {
        BlockState {
            block_hash,
            predecessor_block_hash: None,
            block_downloaded: true,
            operations_downloaded: true,
            applied: true,
        }
    }

    fn update(&mut self, new_state: &InnerBlockState) -> bool {
        let mut was_updated = false;

        if new_state.block_downloaded {
            if !self.block_downloaded {
                self.block_downloaded = true;
                was_updated = true;
            }
        }

        if new_state.applied {
            if !self.applied {
                self.applied = true;
                was_updated = true;
            }
        }

        if new_state.operations_downloaded {
            if !self.operations_downloaded {
                self.operations_downloaded = true;
                was_updated = true;
            }
        }

        was_updated
    }

    fn update_predecessor(&mut self, predecessor: Arc<BlockHash>) -> bool {
        if self.predecessor_block_hash.is_none() {
            self.predecessor_block_hash = Some(predecessor);
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::state::tests::block;

    use super::*;

    macro_rules! hash_set {
        ( $( $x:expr ),* ) => {
            {
                let mut temp_set = HashSet::new();
                $(
                    temp_set.insert($x);
                )*
                temp_set
            }
        };
    }

    #[test]
    fn test_bootstrap_state_split_to_intervals() {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let pipeline = BootstrapState::new(chain_id, last_applied.clone(), history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check applied is just first block
        pipeline
            .intervals
            .iter()
            .map(|i| &i.blocks)
            .flatten()
            .for_each(|b| {
                if b.block_hash.as_ref().eq(last_applied.as_ref()) {
                    assert!(b.applied);
                    assert!(b.block_downloaded);
                    assert!(b.operations_downloaded);
                } else {
                    assert!(!b.applied);
                    assert!(!b.block_downloaded);
                    assert!(!b.operations_downloaded);
                }
            })
    }

    #[test]
    fn test_bootstrap_state_block_downloaded() -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // register downloaded block 2 with his predecessor 1
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 2);
        let result =
            pipeline.find_next_blocks_to_download(5, HashSet::default(), 1, |_| Ok(None))?;
        assert_eq!(1, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());

        pipeline.block_downloaded(
            &block(2),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: false,
            },
            block(1),
        );
        // interval not closed
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 3);

        let result =
            pipeline.find_next_blocks_to_download(5, HashSet::default(), 1, |_| Ok(None))?;
        assert_eq!(1, result.len());
        assert_eq!(result[0].as_ref(), block(1).as_ref());

        // register downloaded block 1
        pipeline.block_downloaded(
            &block(1),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: false,
            },
            block(0),
        );
        // interval is closed
        assert!(pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 3);

        // next interval
        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[1].blocks.len(), 2);

        let result =
            pipeline.find_next_blocks_to_download(5, HashSet::default(), 1, |_| Ok(None))?;
        assert_eq!(1, result.len());
        assert_eq!(result[0].as_ref(), block(5).as_ref());

        // register downloaded block 5 with his predecessor 4
        pipeline.block_downloaded(
            &block(5),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: false,
            },
            block(4),
        );
        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[1].blocks.len(), 3);

        // register downloaded block 4 as applied
        pipeline.block_downloaded(
            &block(4),
            &InnerBlockState {
                block_downloaded: true,
                applied: true,
                operations_downloaded: false,
            },
            block(3),
        );

        // first interval with block 2 and block 3 were removed, because if 4 is applied, 2/3 must be also
        assert!(pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 2);
        assert_eq!(
            pipeline.intervals[0].blocks[0].block_hash.as_ref(),
            block(4).as_ref()
        );
        assert_eq!(
            pipeline.intervals[0].blocks[1].block_hash.as_ref(),
            block(5).as_ref()
        );

        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[1].blocks.len(), 2);
        assert_eq!(
            pipeline.intervals[1].blocks[0].block_hash.as_ref(),
            block(5).as_ref()
        );
        assert_eq!(
            pipeline.intervals[1].blocks[1].block_hash.as_ref(),
            block(8).as_ref()
        );

        Ok(())
    }

    #[test]
    fn test_bootstrap_state_block_applied() {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check intervals
        assert_eq!(pipeline.intervals.len(), 7);

        // check first block (block(2) ) from next 1 interval
        assert!(!pipeline.intervals[1].blocks[0].applied);
        assert!(!pipeline.intervals[1].blocks[0].block_downloaded);
        assert!(!pipeline.intervals[1].blocks[0].operations_downloaded);

        // register downloaded block 2 which is applied
        pipeline.block_downloaded(
            &block(2),
            &InnerBlockState {
                block_downloaded: true,
                applied: true,
                operations_downloaded: false,
            },
            block(1),
        );
        // interval 0 was removed
        assert_eq!(pipeline.intervals.len(), 6);
        // and first block of next interval is marked the same as the last block from 0 inerval, becauase it is the same block (block(2))
        // check first block (block(2) ) from next 1 interval
        assert!(pipeline.intervals[0].blocks[0].applied);
        assert!(pipeline.intervals[0].blocks[0].block_downloaded);
        assert!(!pipeline.intervals[0].blocks[0].operations_downloaded);
    }

    #[test]
    fn test_bootstrap_state_block_applied_marking() {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check intervals
        assert_eq!(pipeline.intervals.len(), 7);

        // trigger that block 2 is download with predecessor 1
        pipeline.block_downloaded(
            &block(2),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: false,
            },
            block(1),
        );
        // mark 1 as applied (half of interval)
        pipeline.block_applied(&block(1));
        assert_eq!(pipeline.intervals.len(), 7);
        // begining of interval is changed to block1
        assert_eq!(pipeline.intervals[0].blocks.len(), 2);
        assert_eq!(
            pipeline.intervals[0].blocks[0].block_hash.as_ref(),
            block(1).as_ref()
        );
        assert_eq!(
            pipeline.intervals[0].blocks[1].block_hash.as_ref(),
            block(2).as_ref()
        );

        // trigger that block 2 is applied
        pipeline.block_applied(&block(2));
        assert_eq!(pipeline.intervals.len(), 6);

        // trigger that block 8 is applied
        pipeline.block_applied(&block(8));
        for (id, i) in pipeline.intervals.iter().enumerate() {
            println!(
                "{} : {:?}",
                id,
                i.blocks
                    .iter()
                    .map(|b| b.block_hash.as_ref().clone())
                    .collect::<Vec<_>>()
            );
        }
        assert_eq!(pipeline.intervals.len(), 4);

        // trigger that last block is applied
        pipeline.block_applied(&block(20));

        println!("inevals: {}", pipeline.intervals.len());

        // interval 0 was removed
        assert!(pipeline.is_done());
    }

    #[test]
    fn test_find_first_missing_block_with_data_downloaded_as_from_other_peers_step_by_step(
    ) -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check - 2 is not downloaded
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(2, pipeline.intervals[0].blocks.len());
        assert!(matches!(
            pipeline.intervals[0].find_first_missing_block(&HashSet::default(), |_| Ok(None))?,
            (Some(next_block), was_updated) if eq(&next_block, &block(2)) && !was_updated
        ));
        assert!(matches!(
            pipeline.intervals[0].find_first_missing_block(&hash_set![block(2)], |_| Ok(None))?,
            (None, was_updated) if !was_updated
        ));

        // check - 2 is not downloaded but is already scheduled
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(2, pipeline.intervals[0].blocks.len());
        assert!(matches!(
            pipeline.intervals[0].find_first_missing_block(&hash_set![block(2)], |_| Ok(None))?,
            (None, was_updated) if !was_updated
        ));

        // check - with 2 was downloaded
        assert!(matches!(
            pipeline.intervals[0].find_first_missing_block(&HashSet::default(), |bh| {
                if bh.eq(&block(2)) {
                    Ok(Some(result(true, false, false, block(1))))
                } else {
                   Ok(None)
                }
            })?,
            (Some(next_block), was_updated) if eq(&next_block, &block(1)) && was_updated
        ));
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(3, pipeline.intervals[0].blocks.len());

        // still waiting for1
        assert!(matches!(
            pipeline.intervals[0].find_first_missing_block(&HashSet::default(), |_| Ok(None))?,
            (Some(next_block), was_updated) if eq(&next_block, &block(1)) && !was_updated
        ));

        // check - with 1 was downloaded
        assert!(matches!(
            pipeline.intervals[0].find_first_missing_block(&HashSet::default(), |bh| {
                if bh.eq(&block(1)) {
                    Ok(Some(result(true, false, false, block(0))))
                } else if bh.eq(&block(0)) {
                   Ok(Some(result(true, true, true, block(0))))
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?,
            (None, was_updated) if was_updated
        ));
        assert!(pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(3, pipeline.intervals[0].blocks.len());

        // check - nothing
        assert!(matches!(
            pipeline.intervals[0].find_first_missing_block(&HashSet::default(), |_| panic!("test failed"))?,
            (None, was_updated) if !was_updated
        ));

        Ok(())
    }

    #[test]
    fn test_find_first_missing_block_with_data_downloaded_as_from_other_peers_in_one_shot(
    ) -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check 2. inerval -  is not downloaded
        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(2, pipeline.intervals[1].blocks.len());

        // get next for download
        assert!(matches!(
            pipeline.intervals[1].find_first_missing_block(&HashSet::default(), |bh| {
                // on backgroung all data were downloaded by other peers
                if bh.eq(&block(5)) {
                    Ok(Some(result(true, false, false, block(4))))
                } else if bh.eq(&block(4)) {
                    Ok(Some(result(true, false, false, block(3))))
                } else if bh.eq(&block(3)) {
                    Ok(Some(result(true, false, false, block(2))))
                } else if bh.eq(&block(2)) {
                    Ok(Some(result(true, false, false, block(1))))
                } else if bh.eq(&block(1)) {
                    Ok(Some(result(true, false, false, block(0))))
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?,
            (None, was_updated) if was_updated
        ));

        // all resolved as downloaded
        assert!(pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(4, pipeline.intervals[1].blocks.len());

        Ok(())
    }

    #[test]
    fn test_find_first_missing_block_with_data_downloaded_as_from_other_peers_in_one_shot_not_closed_interval(
    ) -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check 2. inerval -  is not downloaded
        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(2, pipeline.intervals[1].blocks.len());

        // get next for download
        assert!(matches!(
            pipeline.intervals[1].find_first_missing_block(&HashSet::default(), |bh| {
                if bh.eq(&block(5)) {
                    Ok(Some(result(true, false, false, block(4))))
                } else if bh.eq(&block(4)) {
                    Ok(Some(result(true, false, false, block(3))))
                } else if bh.eq(&block(3)) {
                    // three not downloaded
                    Ok(None)
                } else if bh.eq(&block(2)) {
                    Ok(Some(result(true, false, false, block(1))))
                } else if bh.eq(&block(1)) {
                    Ok(Some(result(true, false, false, block(0))))
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?,
            (Some(next_block), was_updated) if eq(&next_block, &block(3)) && was_updated
        ));

        // all resolved as downloaded
        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(4, pipeline.intervals[1].blocks.len());

        Ok(())
    }

    #[test]
    fn test_find_next_blocks_to_download() -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check 2. inerval -  is not downloaded
        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(2, pipeline.intervals[1].blocks.len());

        // try to get blocks for download - max 1 interval
        let result =
            pipeline.find_next_blocks_to_download(5, HashSet::default(), 1, |_| Ok(None))?;
        assert_eq!(1, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());

        // try to get blocks for download - max 2 interval
        let result =
            pipeline.find_next_blocks_to_download(5, HashSet::default(), 2, |_| Ok(None))?;
        assert_eq!(2, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());
        assert_eq!(result[1].as_ref(), block(5).as_ref());

        // try to get blocks for download - max 2 interval with ignored
        let result =
            pipeline.find_next_blocks_to_download(5, hash_set![block(5)], 2, |_| Ok(None))?;
        assert_eq!(2, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());
        assert_eq!(result[1].as_ref(), block(8).as_ref());

        // try to get blocks for download - max 4 interval
        let result =
            pipeline.find_next_blocks_to_download(5, HashSet::default(), 4, |_| Ok(None))?;
        assert_eq!(4, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());
        assert_eq!(result[1].as_ref(), block(5).as_ref());
        assert_eq!(result[2].as_ref(), block(8).as_ref());
        assert_eq!(result[3].as_ref(), block(10).as_ref());

        Ok(())
    }

    #[test]
    fn test_find_block_with_missing_operations() -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check 1. inerval -  is not downloaded
        assert!(!pipeline.intervals[0].all_operations_downloaded);

        // download block 2 (but operations still misssing)
        pipeline.block_downloaded(
            &block(2),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: false,
            },
            block(1),
        );

        // get next for download (meanwhile operations for 2 was downloaded)
        let (missing_blocks, was_updated) = pipeline.intervals[0]
            .find_block_with_missing_operations(5, &HashSet::default(), |bh| {
                if bh.eq(&block(2)) {
                    Ok(true)
                } else if bh.eq(&block(1)) {
                    Ok(false)
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?;
        assert!(was_updated);
        assert_eq!(1, missing_blocks.len());
        assert_eq!(missing_blocks[0], block(1));
        assert!(!pipeline.intervals[0].all_operations_downloaded);

        // get next for download with ignored blocks
        let (missing_blocks, was_updated) = pipeline.intervals[0]
            .find_block_with_missing_operations(5, &hash_set![block(1)], |bh| {
                panic!("test failed: {:?}", bh)
            })?;
        assert!(!was_updated);
        assert_eq!(0, missing_blocks.len());
        assert!(!pipeline.intervals[0].all_operations_downloaded);

        // download block 1 (but operations still misssing)
        pipeline.block_downloaded(
            &block(1),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: false,
            },
            block(0),
        );

        // get next for download -- all operations downloaded
        let (missing_blocks, was_updated) = pipeline.intervals[0]
            .find_block_with_missing_operations(5, &HashSet::default(), |bh| {
                if bh.eq(&block(1)) {
                    Ok(true)
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?;
        assert!(was_updated);
        assert_eq!(0, missing_blocks.len());
        assert!(pipeline.intervals[0].all_operations_downloaded);

        Ok(())
    }

    #[test]
    fn test_find_next_block_operations_to_download() -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check 2. inerval -  is not downloaded
        assert!(!pipeline.intervals[1].all_blocks_downloaded);
        assert_eq!(2, pipeline.intervals[1].blocks.len());

        // try to get blocks for download - max 1 interval
        let result =
            pipeline
                .find_next_block_operations_to_download(5, HashSet::default(), 1, |_| Ok(false))?;
        assert_eq!(1, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());

        // try to get blocks for download - max 2 interval
        let result =
            pipeline
                .find_next_block_operations_to_download(5, HashSet::default(), 2, |_| Ok(false))?;
        assert_eq!(2, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());
        assert_eq!(result[1].as_ref(), block(5).as_ref());

        // try to get blocks for download - max 4 interval
        let result =
            pipeline
                .find_next_block_operations_to_download(5, HashSet::default(), 4, |_| Ok(false))?;
        assert_eq!(4, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());
        assert_eq!(result[1].as_ref(), block(5).as_ref());
        assert_eq!(result[2].as_ref(), block(8).as_ref());
        assert_eq!(result[3].as_ref(), block(10).as_ref());

        // try to get blocks for download - max 4 interval with ignored
        let result =
            pipeline.find_next_block_operations_to_download(5, hash_set![block(10)], 4, |_| {
                Ok(false)
            })?;
        assert_eq!(4, result.len());
        assert_eq!(result[0].as_ref(), block(2).as_ref());
        assert_eq!(result[1].as_ref(), block(5).as_ref());
        assert_eq!(result[2].as_ref(), block(8).as_ref());
        assert_eq!(result[3].as_ref(), block(13).as_ref());

        Ok(())
    }

    #[test]
    fn test_find_next_block_to_apply() -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // check begining of the pipeleines, shoudl be last_applied
        assert_eq!(
            pipeline.intervals[0].blocks[0].block_hash.as_ref(),
            block(0).as_ref()
        );
        assert!(pipeline.intervals[0].blocks[0].applied);

        // try to get next block to apply (no data were downloaded before), so nothging to apply
        assert!(matches!(
            pipeline.find_next_block_to_apply(0, |bh| {
                if bh.eq(&block(2)) {
                    Ok(false)
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?,
            None
        ));

        // another try, but block 2 was applied meanwhile
        assert!(matches!(
            pipeline.find_next_block_to_apply(0, |bh| {
                if bh.eq(&block(2)) {
                    // was applied
                    Ok(true)
                } else if bh.eq(&block(5)) {
                    // was applied
                    Ok(false)
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?,
            None
        ));

        // so first interval should be removed now and all other moved
        assert_eq!(pipeline.intervals.len(), 6);
        assert_interval(&pipeline.intervals[0], (block(2), block(5)));
        assert_eq!(
            pipeline.intervals[0].blocks[0].block_hash.as_ref(),
            block(2).as_ref()
        );
        assert!(pipeline.intervals[0].blocks[0].applied);

        // another try, but nothging
        assert!(matches!(
            pipeline.find_next_block_to_apply(0, |bh| {
                if bh.eq(&block(5)) {
                    // was applied
                    Ok(false)
                } else {
                    panic!("test failed: {:?}", bh)
                }
            })?,
            None
        ));

        Ok(())
    }

    #[test]
    fn test_download_all_blocks_and_operations() {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // download blocks and operations from 0 to 8
        pipeline.block_downloaded(
            &block(8),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(7),
        );
        pipeline.block_downloaded(
            &block(7),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(6),
        );
        pipeline.block_downloaded(
            &block(6),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(5),
        );
        pipeline.block_downloaded(
            &block(5),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(4),
        );
        pipeline.block_downloaded(
            &block(4),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(3),
        );
        pipeline.block_downloaded(
            &block(3),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(2),
        );
        pipeline.block_downloaded(
            &block(2),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(1),
        );
        pipeline.block_downloaded(
            &block(1),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(0),
        );

        // check all downloaded inervals
        assert!(pipeline.intervals[0].all_blocks_downloaded);
        assert!(pipeline.intervals[0].all_operations_downloaded);
        assert!(pipeline.intervals[1].all_blocks_downloaded);
        assert!(pipeline.intervals[1].all_operations_downloaded);
    }

    #[test]
    fn test_find_next_block_to_apply_batch() -> Result<(), StateError> {
        // genesis
        let last_applied = block(0);
        // history blocks
        let history: Vec<Arc<BlockHash>> = vec![
            block(2),
            block(5),
            block(8),
            block(10),
            block(13),
            block(15),
            block(20),
        ];
        let chain_id = Arc::new(
            ChainId::from_base58_check("NetXgtSLGNJvNye").expect("Failed to create chainId"),
        );

        // create
        let mut pipeline = BootstrapState::new(chain_id, last_applied, history, Arc::new(20));
        assert_eq!(pipeline.intervals.len(), 7);
        assert_interval(&pipeline.intervals[0], (block(0), block(2)));
        assert_interval(&pipeline.intervals[1], (block(2), block(5)));
        assert_interval(&pipeline.intervals[2], (block(5), block(8)));
        assert_interval(&pipeline.intervals[3], (block(8), block(10)));
        assert_interval(&pipeline.intervals[4], (block(10), block(13)));
        assert_interval(&pipeline.intervals[5], (block(13), block(15)));
        assert_interval(&pipeline.intervals[6], (block(15), block(20)));

        // download blocks and operations from 0 to 8
        pipeline.block_downloaded(
            &block(8),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(7),
        );
        pipeline.block_downloaded(
            &block(7),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(6),
        );
        pipeline.block_downloaded(
            &block(6),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(5),
        );
        pipeline.block_downloaded(
            &block(5),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(4),
        );
        pipeline.block_downloaded(
            &block(4),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(3),
        );
        pipeline.block_downloaded(
            &block(3),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(2),
        );
        pipeline.block_downloaded(
            &block(2),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(1),
        );
        pipeline.block_downloaded(
            &block(1),
            &InnerBlockState {
                block_downloaded: true,
                applied: false,
                operations_downloaded: true,
            },
            block(0),
        );

        // check all downloaded inervals
        assert!(pipeline.intervals[0].all_blocks_downloaded);
        assert!(pipeline.intervals[0].all_operations_downloaded);
        assert!(pipeline.intervals[1].all_blocks_downloaded);
        assert!(pipeline.intervals[1].all_operations_downloaded);

        // next for apply with max batch 0
        let next_batch = pipeline.find_next_block_to_apply(0, |bh| {
            if bh.eq(&block(1)) {
                // was not applied
                Ok(false)
            } else {
                panic!("test failed: {:?}", bh)
            }
        })?;
        assert!(next_batch.is_some());
        let next_batch = next_batch.unwrap();
        assert_eq!(next_batch.block_to_apply.as_ref(), block(1).as_ref());
        assert_eq!(0, next_batch.successors_size());

        // next for apply with max batch 1
        let next_batch = pipeline.find_next_block_to_apply(1, |bh| {
            if bh.eq(&block(1)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(2)) {
                // was not applied
                Ok(false)
            } else {
                panic!("test failed: {:?}", bh)
            }
        })?;
        assert!(next_batch.is_some());
        let next_batch = next_batch.unwrap();
        assert_eq!(next_batch.block_to_apply.as_ref(), block(1).as_ref());
        assert_eq!(1, next_batch.successors_size());
        assert_eq!(next_batch.successors[0].as_ref(), block(2).as_ref());

        // next for apply with max batch 100
        let next_batch = pipeline.find_next_block_to_apply(100, |bh| {
            if bh.eq(&block(1)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(2)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(3)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(4)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(5)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(6)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(7)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(8)) {
                // was not applied
                Ok(false)
            } else if bh.eq(&block(10)) {
                // was not applied
                Ok(false)
            } else {
                panic!("test failed: {:?}", bh)
            }
        })?;
        assert!(next_batch.is_some());
        let next_batch = next_batch.unwrap();
        assert_eq!(next_batch.block_to_apply.as_ref(), block(1).as_ref());
        assert_eq!(7, next_batch.successors_size());
        assert_eq!(next_batch.successors[0].as_ref(), block(2).as_ref());
        assert_eq!(next_batch.successors[1].as_ref(), block(3).as_ref());
        assert_eq!(next_batch.successors[2].as_ref(), block(4).as_ref());
        assert_eq!(next_batch.successors[3].as_ref(), block(5).as_ref());
        assert_eq!(next_batch.successors[4].as_ref(), block(6).as_ref());
        assert_eq!(next_batch.successors[5].as_ref(), block(7).as_ref());
        assert_eq!(next_batch.successors[6].as_ref(), block(8).as_ref());

        Ok(())
    }

    fn eq(b1: &BlockHash, b2: &BlockHash) -> bool {
        b1.eq(b2)
    }

    fn result(
        block_downloaded: bool,
        applied: bool,
        operations_downloaded: bool,
        block_hash: Arc<BlockHash>,
    ) -> (InnerBlockState, Arc<BlockHash>) {
        (
            InnerBlockState {
                block_downloaded,
                applied,
                operations_downloaded,
            },
            block_hash,
        )
    }

    fn assert_interval(
        tested: &BootstrapInterval,
        (expected_left, expected_right): (Arc<BlockHash>, Arc<BlockHash>),
    ) {
        assert_eq!(tested.blocks.len(), 2);
        assert_eq!(tested.blocks[0].block_hash.as_ref(), expected_left.as_ref());
        assert_eq!(
            tested.blocks[1].block_hash.as_ref(),
            expected_right.as_ref()
        );
    }
}
