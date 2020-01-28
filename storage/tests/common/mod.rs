// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::{HashMap, HashSet};
use std::iter::FromIterator;

use serde::{Deserialize, Serialize};

use storage::persistent::BincodeEncoded;
use storage::skip_list::ListValue;

#[derive(Default, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Value(HashSet<u64>);

impl Value {
    pub fn new(value: Vec<u64>) -> Self {
        Self(HashSet::from_iter(value))
    }
}

impl ListValue<u64, u64> for Value {
    fn merge(&mut self, other: &Self) {
        self.0.extend(&other.0)
    }

    fn diff(&mut self, other: &Self) {
        for x in &other.0 {
            if !self.0.contains(x) {
                self.0.insert(*x);
            }
        }
    }

    fn get(&self, value: &u64) -> Option<u64> {
        self.0.get(value).map(|v| v.clone())
    }
}

impl BincodeEncoded for Value {}

#[derive(Default, PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
pub struct OrderedValue(pub HashMap<u64, u64>);

impl OrderedValue {
    #[allow(dead_code)]
    pub fn new(value: HashMap<u64, u64>) -> Self {
        Self(value)
    }
}

impl ListValue<u64, u64> for OrderedValue {
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

    fn get(&self, value: &u64) -> Option<u64> {
        self.0.get(value).map(|v| v.clone())
    }
}

impl BincodeEncoded for OrderedValue {}