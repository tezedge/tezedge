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
#![allow(dead_code)]

mod lane;
mod content;
mod skip_list;

pub use skip_list::SkipList;

pub const LEVEL_BASE: usize = 8;

#[cfg(test)]
pub mod common_testing {
    use storage::persistent::{open_db, Schema, BincodeEncoded};
    use std::{
        sync::Arc,
        process::Command,
    };
    use serde::{Serialize, Deserialize};
    use rocksdb::DB;
    use crate::lane::Lane;
    use crate::content::ListValue;
    use std::collections::{HashSet, HashMap};
    use std::iter::FromIterator;
    use std::fs::remove_dir_all;

    pub struct TmpDb {
        db: Arc<DB>,
        tmp_dir: String,
    }

    impl TmpDb {
        pub fn new() -> Self {
            let proc = Command::new("mktemp").args(&["-d"]).output();
            let dir = String::from_utf8(proc.unwrap().stdout)
                .expect("failed to create testing database").trim().to_string();
            let db = open_db(&dir, vec![Lane::<Value>::cf_descriptor()]).unwrap();
            Self {
                db: Arc::new(db),
                tmp_dir: dir,
            }
        }

        pub fn db(&self) -> Arc<DB> {
            self.db.clone()
        }
    }

    impl Drop for TmpDb {
        fn drop(&mut self) {
            remove_dir_all(&self.tmp_dir)
                .expect("Failed to remove temp database directory");
        }
    }

    #[derive(Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
    pub struct Value(pub HashSet<usize>);

    impl Value {
        pub fn new(value: Vec<usize>) -> Self {
            Self(HashSet::from_iter(value))
        }
    }

    impl ListValue for Value {
        /// Merge two sets
        fn merge(&mut self, other: &Self) {
            self.0.extend(&other.0)
        }

        /// Create the
        fn diff(&mut self, other: &Self) {
            for x in &other.0 {
                if !self.0.contains(x) {
                    self.0.insert(*x);
                }
            }
        }
    }

    impl BincodeEncoded for Value {}

    #[derive(Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
    pub struct OrderedValue(pub HashMap<usize, usize>);

    impl OrderedValue {
        pub fn new(value: HashMap<usize, usize>) -> Self {
            Self(value)
        }
    }

    impl ListValue for OrderedValue {
        /// Merge two sets
        fn merge(&mut self, other: &Self) {
            self.0.extend(&other.0)
        }

        /// Create the
        fn diff(&mut self, other: &Self) {
            for (k, v) in &other.0 {
                if !self.0.contains_key(k) {
                    self.0.insert(*k, *v);
                }
            }
        }
    }

    impl BincodeEncoded for OrderedValue {}
}