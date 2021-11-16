// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Clone, Copy)]
pub struct RequestId {
    locator: usize,
    counter: usize,
}

impl RequestId {
    pub(super) fn new(locator: usize, counter: usize) -> Self {
        Self { locator, counter }
    }

    pub fn locator(&self) -> usize {
        self.locator
    }

    pub fn counter(&self) -> usize {
        self.counter
    }
}
