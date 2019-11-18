// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Experimental data structure for faster state-change traversing and state re-creation.
//! Basic idea is based on single-way expanding skip-list. As block-chain does not required
//! insertion in middle of block-chain, there are a few optimization, we can apply to improve
//! performance.
//!
//! Structure contains a multiple parallel "lanes". The lowest lane works exactly as a linked list,
//! and each higher lane skips more nodes.
//!
//! L2: ┌──────────────────────S015─────────────────────────┐
//! L1: ┌───S03───┐→┌───S47───┐→┌────S811───┐→┌────S1215────┐
//! L0: S0→S1→S2→S3→S4→S5→S6→S7→S8→S9→S10→S11→S12→S13→S14→S15
//!
//! * Rebuilding state after applying first 3 blocks, require sequential application of changes
//! {S0 → S1 → S2}.
//! * State re-creation for first 16 blocks can be done simply by traversing faster lanes (L1), and applying
//! aggregated changes on lane descend {S015, S1215}.
#[allow(dead_code)]

mod lane;
mod content;



///// Column family for storing basic data to orientate on data structure.
//struct SkipList<C: ListValue> {
//    container: Arc<DB>,
//    levels: usize,
//    len: usize,
//    _pd: PhantomData<C>,
//}
//
//impl<C: ListValue> Schema for SkipList<C> {
//    // TODO: This should be somewhat changed, to accomodate for more than one usage of this structure
//    // TODO: in code
//    const COLUMN_FAMILY_NAME: &'static str = "skip_list";
//    type Key = NodeHeader;
//    type Value = C;
//}
//
//impl<C: ListValue> SkipList<C> {
//    pub fn new(db: Arc<DB>) -> Self {
//        Self {
//            container: db,
//            levels: 1,
//            len: 0,
//            _pd: PhantomData,
//        }
//    }
//
//    pub fn len(&self) -> usize {
//        self.len
//    }
//
//    pub fn contains(&self, index: usize) -> bool {
//        self.len > index
//    }
//
//    pub fn get(&self, index: usize) -> Option<C> {
//        let mut current_level = self.levels - 1;
//        let mut current_state: Option<C> = None;
//        let mut current_pos = 0;
//        let mut current_index = 0;
//        let mut lane: Lane = Lane::new(self.levels, self.container.clone());
//
//        loop {
//            let step = Self::level_step(current_level);
//            if current_pos + step > index {
//                // Descend a level
//                if current_level == 0 {
//                    return current_state;
//                } else {
//                    current_index += 1;
//                    current_pos += step;
//                }
//            } else {
//                // Continue
//            }
//        }
//    }
//
//    #[inline]
//    fn level_step(level: usize) -> usize {
//        LEVEL_BASE.pow(level)
//    }
//}