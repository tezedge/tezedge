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
//! L2: ┌─────────────────────S015──────────────────────────┐
//! L1: ┌───S03───┐→┌───S47───┐→┌────S811───┐→┌────S1215────┐
//! L0: S0→S1→S2→S3→S4→S5→S6→S7→S8→S9→S10→S11→S12→S13→S14→S15
//!
//! * Rebuilding state after applying first 3 blocks, require sequential application of changes
//! {S0 → S1 → S2}.
//! * State re-creation for first 16 blocks can be done simply by traversing faster lanes (L1), and applying
//! aggregated changes on lane descend {S015, S1215}.

pub use crate::skip_list::content::{Bucket, ListValue, SkipListError};
pub use crate::skip_list::lane::{Lane, TypedLane};
pub use crate::skip_list::list::{DatabaseBackedSkipList, SkipList, TypedSkipList};

mod content;
mod lane;
mod list;

pub(crate) const LEVEL_BASE: usize = 8;

pub trait TryExtend<A> {
    fn try_extend<T: IntoIterator<Item = A>>(&mut self, iter: T) -> Result<(), SkipListError>;
}
