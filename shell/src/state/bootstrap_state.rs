// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Temporary bootstrap state which helps to download and validate branches from peers
//!
//! - every peer has his own BootstrapState
//! - bootstrap state is initialized from branch history, which is splitted to partitions

use std::sync::Arc;
use std::time::{Duration, Instant};

use crypto::hash::{BlockHash, ChainId};
use tezos_messages::p2p::encoding::block_header::Level;

use crate::state::StateError;

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

    /// This finds block, which should be downloaded first.
    pub fn next_block_to_download(&self) -> Option<Arc<BlockHash>> {
        let mut previous_interval_applied = true;

        for interval in &self.intervals {
            // Note: little slow down, we want to download next interval, only if the previos one was applied successfully
            // to prevent downloading invalid rest of the chain
            if !previous_interval_applied {
                break;
            }
            previous_interval_applied = interval.all_block_applied;

            // if interval is downloaded, just skip it
            if interval.all_blocks_downloaded {
                continue;
            }

            // get first non-downloaded block
            for b in &interval.blocks {
                // skip applied
                if b.applied {
                    continue;
                }

                if !b.block_downloaded {
                    return Some(b.block_hash.clone());
                }
            }

            // if we came here, it means, that interval is not closed, but all blocks are downloaded
            // we mark all_blocks_downloaded only if predecessor matches first block
            // so we need to return here first block after begining to continue
            // to prevent stucking the pipeline
            if let Some(first_block_after_begining) = interval.blocks.get(1) {
                return Some(first_block_after_begining.block_hash.clone());
            }
        }
        None
    }

    /// This finds block, for which we should download operations.
    pub fn next_block_operations_to_download(&self) -> Option<Arc<BlockHash>> {
        let mut previous_interval_applied = true;

        for interval in &self.intervals {
            // Note: little slow down, we want to download next interval, only if the previos one was applied successfully
            // to prevent downloading invalid rest of the chain
            if !previous_interval_applied {
                break;
            }
            previous_interval_applied = interval.all_block_applied;

            // get first non-downloaded block
            for b in &interval.blocks {
                // skip applied
                if b.applied {
                    continue;
                }

                if !b.operations_downloaded {
                    return Some(b.block_hash.clone());
                }
            }
        }
        None
    }

    /// This finds block, for which we should apply.
    pub fn next_block_to_apply(&self) -> Option<Arc<BlockHash>> {
        let mut previous_interval_applied = true;

        for interval in &self.intervals {
            // Note: little slow down, we want to download next interval, only if the previos one was applied successfully
            // to prevent downloading invalid rest of the chain
            if !previous_interval_applied {
                break;
            }
            previous_interval_applied = interval.all_block_applied;

            // get first non-applied block
            for b in &interval.blocks {
                // skip applied
                if b.applied {
                    continue;
                }

                // just check first block
                if b.block_downloaded && b.operations_downloaded {
                    return Some(b.block_hash.clone());
                } else {
                    return None;
                }
            }
        }
        None
    }

    /// Notify state requested_block_hash was downloaded
    ///
    /// <BM> callback wchic return (bool as downloaded, bool as applied, bool as are_operations_complete)
    pub fn block_downloaded<BM>(
        &mut self,
        requested_block_hash: &BlockHash,
        requested_block_new_inner_state: &InnerBlockState,
        predecessor_block_hash: &BlockHash,
        get_block_metadata: BM,
    ) -> Result<(), StateError>
    where
        BM: Fn(&BlockHash) -> Result<Option<InnerBlockState>, StateError>,
    {
        // find first interval with this block
        let interval_to_handle = self.intervals.iter_mut().enumerate().find(|(_, interval)| {
            // find first interval with this block
            interval
                .blocks
                .iter()
                .filter(|b| !b.applied)
                .any(|b| b.block_hash.as_ref().eq(requested_block_hash))
        });

        let mut is_predecessor_applied = false;

        // Note:end_interval
        let mut end_of_interval: Option<(usize, InnerBlockState)> = None;

        if let Some((interaval_idx, interval)) = interval_to_handle {
            let mut predecessor_insert_to_index: Option<usize> = None;
            let mut predecessor_found = false;
            let block_count = interval.blocks.len();
            // update metadata
            for (block_index, b) in interval.blocks.iter_mut().enumerate() {
                // Note:begining_interval - handle
                // we found predecessor, means we found begining of interval
                if predecessor_block_hash.eq(b.block_hash.as_ref()) {
                    // Note:begining_interval - handle begining of the interval
                    predecessor_found = true;

                    // if block is not applied, then try to update his state
                    // Note: we expect that block here is applied
                    if !b.applied {
                        // update its state
                        if let Some(new_state) = get_block_metadata(predecessor_block_hash)? {
                            if b.update(&new_state) {
                                self.last_updated = Instant::now();
                            }
                        }
                    }

                    // Note:begining_interval - handle
                    // if predecessor (a.k.a begining of the interval) is downloaded, it means that whole interval is downloaded
                    if b.block_downloaded && block_index == 0 {
                        interval.all_blocks_downloaded = true;
                        self.last_updated = Instant::now();
                    }

                    if b.applied {
                        is_predecessor_applied = true;
                    }
                }

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

                    // add predecessor to pipeline, if not found yet
                    if !predecessor_found {
                        // predecessor_found = true;
                        // insert before requested block
                        predecessor_insert_to_index = Some(block_index);
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

            // if we need to insert predecessor, measn we are not done, and interval is not closed yet
            if let Some(predecessor_index) = predecessor_insert_to_index {
                // create actual state
                let mut predecessor_state =
                    BlockState::new(Arc::new(predecessor_block_hash.clone()));
                if let Some(new_state) = &get_block_metadata(predecessor_block_hash)? {
                    let _ = predecessor_state.update(&new_state);
                }

                if predecessor_state.applied {
                    is_predecessor_applied = true;
                } else {
                    // insert
                    interval.blocks.insert(predecessor_index, predecessor_state);
                    self.last_updated = Instant::now();
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

        // handle, what if requested block is already applied (this removes all predecessor)
        if requested_block_new_inner_state.applied {
            self.block_applied(&requested_block_hash);
            return Ok(());
        }
        if is_predecessor_applied {
            self.block_applied(&predecessor_block_hash);
            return Ok(());
        }

        Ok(())
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
            }
        };
    }

    pub fn is_stalled(&self, stale_timeout: Duration) -> bool {
        self.last_updated.elapsed() > stale_timeout
    }

    pub fn is_done(&self) -> bool {
        self.intervals.is_empty()
    }

    // pub fn state_to_string(&self) -> String {
    //     let intervals = self
    //         .intervals
    //         .iter()
    //         .map(|i| {
    //             let mut block_downloaded = 0;
    //             let mut operations_downloaded = 0;
    //             let mut applied = 0;
    //             for b in &i.blocks {
    //                 if b.block_downloaded {
    //                     block_downloaded += 1;
    //                 }
    //                 if b.operations_downloaded {
    //                     operations_downloaded += 1;
    //                 }
    //                 if b.applied {
    //                     applied += 1;
    //                 }
    //             }
    //             format!(
    //                 "({}/{}, bd: {}, opd: {}, ap: {})",
    //                 i.blocks.len(),
    //                 i.all_blocks_downloaded,
    //                 block_downloaded,
    //                 operations_downloaded,
    //                 applied
    //             )
    //         })
    //         .collect::<Vec<String>>()
    //         .join(", ");
    //
    //     format!(
    //         "{:?}, intervals: {} - {}",
    //         self.last_updated.elapsed(),
    //         self.intervals.len(),
    //         intervals
    //     )
    // }
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
    all_block_applied: bool,

    // TODO: we should maybe add here: (start: Arc<Mutex<BlockState>>, end: Arc<Mutex<BlockState>>)
    //       it would simplify maybe handling see: Note:begining_interval, Note:end_interval
    blocks: Vec<BlockState>,
}

impl BootstrapInterval {
    fn new(block_hash: Arc<BlockHash>) -> Self {
        Self {
            all_blocks_downloaded: false,
            all_block_applied: false,
            blocks: vec![BlockState::new(block_hash)],
        }
    }

    fn new_applied(block_hash: Arc<BlockHash>) -> Self {
        Self {
            all_blocks_downloaded: false,
            all_block_applied: false,
            blocks: vec![BlockState::new_applied(block_hash)],
        }
    }

    fn new_with_left(left: BlockState, right_block_hash: Arc<BlockHash>) -> Self {
        Self {
            all_blocks_downloaded: false,
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
    applied: bool,
    block_downloaded: bool,
    operations_downloaded: bool,
}

impl BlockState {
    fn new(block_hash: Arc<BlockHash>) -> Self {
        BlockState {
            block_hash,
            block_downloaded: false,
            operations_downloaded: false,
            applied: false,
        }
    }

    fn new_applied(block_hash: Arc<BlockHash>) -> Self {
        BlockState {
            block_hash,
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
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use super::*;

    #[test]
    fn test_bootstrap_state_split() {
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
    fn test_bootstrap_state_blocks_downloading() {
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

        // try get next blocks to download
        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(2).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(2).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_operations_to_download(), Some(x) if x.as_ref().eq(block(2).as_ref()))
        );

        // 1. scenario - that predecessor is not downloaded
        // register downloaded block 2 with his predecessor 1 (1 IS NOT downloaded yet)
        assert!(pipeline
            .block_downloaded(
                &block(2),
                &InnerBlockState {
                    block_downloaded: true,
                    applied: false,
                    operations_downloaded: false,
                },
                &block(1),
                |_| {
                    // predecessor IS NOT downloaded yet
                    Ok(Some(InnerBlockState {
                        block_downloaded: false,
                        applied: false,
                        operations_downloaded: false,
                    }))
                },
            )
            .is_ok());
        // interval not closed
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 3);
        // try get next blocks to download, should be 1
        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(1).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(1).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_operations_to_download(), Some(x) if x.as_ref().eq(block(1).as_ref()))
        );
        assert!(matches!(pipeline.next_block_to_apply(), None));

        // download 1 block with operations
        assert!(
            pipeline.block_downloaded(
                &block(1),
                &InnerBlockState {
                    block_downloaded: true,
                    applied: false,
                    operations_downloaded: true,
                },
                &block(0),
                |_| {
                    panic!("Failed - we sould not trigger this, because, we have found begining of interval by 0")
                }).is_ok()
        );
        // interval closed for block downloading
        assert!(pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 3);
        assert!(matches!(pipeline.next_block_to_download(), None));
        assert!(
            matches!(pipeline.next_block_operations_to_download(), Some(x) if x.as_ref().eq(block(2).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_to_apply(), Some(x) if x.as_ref().eq(block(1).as_ref()))
        );

        // close/remove 0. interval
        pipeline.block_operations_downloaded(&block(2));
        pipeline.block_applied(&block(1));
        pipeline.block_applied(&block(2));

        // 2. scenario - that predecessor is downloaded yet (by an other peer's bootstrap_pipeline)
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 2);
        // begining was marked as downloaded by end of the previos interval
        assert_eq!(pipeline.intervals[0].blocks[0].block_downloaded, true);
        assert_eq!(pipeline.intervals[0].blocks[0].applied, true);
        assert_eq!(pipeline.intervals[0].blocks[0].operations_downloaded, true);

        // register downloaded block 5 with his predecessor 4 (4 IS downloaded yet)
        assert!(pipeline
            .block_downloaded(
                &block(5),
                &InnerBlockState {
                    block_downloaded: true,
                    applied: false,
                    operations_downloaded: false,
                },
                &block(4),
                |_| {
                    // predecessor IS downloaded yet
                    Ok(Some(InnerBlockState {
                        block_downloaded: true,
                        applied: false,
                        operations_downloaded: false,
                    }))
                },
            )
            .is_ok());
        assert!(!pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 3);

        // interal is not closed and next block to download will be 4, becauase we did not close interval yet
        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(4).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(4).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_operations_to_download(), Some(x) if x.as_ref().eq(block(4).as_ref()))
        );

        // download 4 without operations
        assert!(pipeline
            .block_downloaded(
                &block(4),
                &InnerBlockState {
                    block_downloaded: true,
                    applied: false,
                    operations_downloaded: false,
                },
                &block(3),
                |_| {
                    // predecessor IS downloaded yet
                    Ok(Some(InnerBlockState {
                        block_downloaded: false,
                        applied: false,
                        operations_downloaded: false,
                    }))
                },
            )
            .is_ok());
        // download 3 without operations
        assert!(pipeline
            .block_downloaded(
                &block(3),
                &InnerBlockState {
                    block_downloaded: true,
                    applied: false,
                    operations_downloaded: false,
                },
                &block(2),
                |_| {
                    // predecessor IS downloaded yet
                    Ok(Some(InnerBlockState {
                        block_downloaded: false,
                        applied: false,
                        operations_downloaded: false,
                    }))
                },
            )
            .is_ok());
        // now is closed
        assert!(pipeline.intervals[0].all_blocks_downloaded);
        assert_eq!(pipeline.intervals[0].blocks.len(), 4);

        assert!(matches!(pipeline.next_block_to_download(), None));
        assert!(
            matches!(pipeline.next_block_operations_to_download(), Some(x) if x.as_ref().eq(block(3).as_ref()))
        );
        assert!(matches!(pipeline.next_block_to_apply(), None));
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

        // try get next blocks to download
        assert_eq!(pipeline.intervals.len(), 7);
        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(2).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_operations_to_download(), Some(x) if x.as_ref().eq(block(2).as_ref()))
        );
        assert!(matches!(pipeline.next_block_to_apply(), None));

        // check first block (block(2) ) from next 1 interval
        assert!(!pipeline.intervals[1].blocks[0].applied);
        assert!(!pipeline.intervals[1].blocks[0].block_downloaded);
        assert!(!pipeline.intervals[1].blocks[0].operations_downloaded);

        // register downloaded block 2 which is applied
        assert!(pipeline
            .block_downloaded(
                &block(2),
                &InnerBlockState {
                    block_downloaded: true,
                    applied: true,
                    operations_downloaded: false,
                },
                &block(1),
                |_| {
                    Ok(Some(InnerBlockState {
                        block_downloaded: true,
                        applied: true,
                        operations_downloaded: true,
                    }))
                },
            )
            .is_ok());
        // interval 0 was removed
        assert_eq!(pipeline.intervals.len(), 6);
        // and first block of next interval is marked the same as the last block from 0 inerval, becauase it is the same block (block(2))
        // check first block (block(2) ) from next 1 interval
        assert!(pipeline.intervals[0].blocks[0].applied);
        assert!(pipeline.intervals[0].blocks[0].block_downloaded);
        assert!(!pipeline.intervals[0].blocks[0].operations_downloaded);

        assert!(
            matches!(pipeline.next_block_to_download(), Some(x) if x.as_ref().eq(block(5).as_ref()))
        );
        assert!(
            matches!(pipeline.next_block_operations_to_download(), Some(x) if x.as_ref().eq(block(5).as_ref()))
        );
        assert!(matches!(pipeline.next_block_to_apply(), None));
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

    fn block(d: u8) -> Arc<BlockHash> {
        Arc::new(
            [d; crypto::hash::HashType::BlockHash.size()]
                .to_vec()
                .try_into()
                .expect("Failed to create BlockHash"),
        )
    }
}
