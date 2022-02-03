// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Clone, Copy)]
pub struct ProtocolRunnerToken(usize);

impl ProtocolRunnerToken {
    /// Caller must make sure token is correct and is associated with
    /// some protocol runner connection.
    #[inline(always)]
    pub fn new_unchecked(token: usize) -> Self {
        Self(token)
    }

    #[inline(always)]
    pub fn index(&self) -> usize {
        self.0
    }
}

impl From<ProtocolRunnerToken> for usize {
    fn from(val: ProtocolRunnerToken) -> usize {
        val.0
    }
}

impl From<ProtocolRunnerToken> for mio::Token {
    fn from(val: ProtocolRunnerToken) -> mio::Token {
        mio::Token(val.0)
    }
}
