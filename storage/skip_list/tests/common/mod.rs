// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    process::Command,
    sync::Arc,
};
use std::collections::{HashMap, HashSet};
use std::fs::remove_dir_all;
use std::iter::FromIterator;

use rocksdb::DB;
use serde::{Deserialize, Serialize};

use skip_list::{Lane, ListValue};
use storage::persistent::{BincodeEncoded, KeyValueSchema, open_db};

pub struct TmpDb {
    db: Arc<DB>,
    tmp_dir: String,
}

impl TmpDb {
    pub fn new() -> Self {
        let proc = Command::new("mktemp").args(&["-d"]).output();
        let dir = String::from_utf8(proc.unwrap().stdout)
            .expect("failed to create testing database").trim().to_string();
        let db = open_db(&dir, vec![Lane::<Value>::descriptor()]).unwrap();
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
pub struct Value(HashSet<usize>);

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
pub struct OrderedValue(HashMap<usize, usize>);

impl OrderedValue {
    #[allow(dead_code)]
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