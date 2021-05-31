// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Tezos binary data reader.

use std::fmt;

use failure::Fail;

/// Error produced by a [BinaryReader].
#[derive(Debug, Clone, Fail)]
pub struct BinaryReaderError(String, bool);

impl BinaryReaderError {
    pub fn is_unsupported_tag(&self) -> bool {
        self.1
    }
}

impl From<String> for BinaryReaderError {
    fn from(error: String) -> Self {
        Self(error, false)
    }
}

impl From<(String, bool)> for BinaryReaderError {
    fn from((error, unsupported_tag): (String, bool)) -> Self {
        Self(error, unsupported_tag)
    }
}

impl fmt::Display for BinaryReaderError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
