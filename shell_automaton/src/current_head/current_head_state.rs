// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{collections::BTreeMap, num::TryFromIntError};

use crypto::{
    hash::{BlockHash, FromBytesError},
    CryptoError,
};
use tezos_encoding::binary_writer::BinaryWriterError;
use tezos_messages::{
    base::signature_public_key::SignaturePublicKey,
    p2p::encoding::block_header::{BlockHeader, Level},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AppliedHead {
    pub level: Level,
    pub block_hash: BlockHash,
    pub timestamp: i64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CurrentHeads {
    pub applied_heads: Vec<AppliedHead>,
    pub applied_hashes: BTreeMap<BlockHash, Level>,
    pub candidates: BTreeMap<BlockHash, CurrentHeadState>,
}

impl CurrentHeads {
    pub(super) fn applied_head(&self) -> Option<&AppliedHead> {
        self.applied_heads.last()
    }

    pub(crate) fn candidate_level(&self) -> Option<Level> {
        self.applied_head().map(|h| h.level + 1)
    }

    pub(crate) fn current_level(&self) -> Option<Level> {
        self.applied_head().map(|h| {
            if self.candidates.is_empty() {
                h.level
            } else {
                h.level + 1
            }
        })
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum CurrentHeadState {
    Received {
        block_header: BlockHeader,
    },
    PendingBakingRights {
        block_header: BlockHeader,
    },
    Prechecked {
        baker: SignaturePublicKey,
        priority: u16,
        block_header: BlockHeader,
    },
    Rejected,
    Error {
        error: CurrentHeadPrecheckError,
    },
}

// ====================

/// Possible current head errors.
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum CurrentHeadPrecheckError {
    #[error(transparent)]
    Priority(#[from] BakingPriorityError),
    #[error(transparent)]
    Rights(#[from] BakingRightsError),
    #[error("{0}")]
    Other(String),
}

// ====================

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum BakingPriorityError {
    #[error("timestamp `{timestamp}` is too far in the future, now is `{now}`")]
    TimeInFuture { now: u64, timestamp: i64 },
    #[error("timestamp `{timestamp}` is before previous timestamp `{prev_timestamp}`")]
    TimeInPast { prev_timestamp: i64, timestamp: i64 },
    #[error("timestamp `{timestamp}` is too early after `{prev_timestamp}`, earliest is {min_timestamp}")]
    TooEarly {
        timestamp: i64,
        prev_timestamp: i64,
        min_timestamp: i64,
    },
    #[error("Too many priorities")]
    Overflow,
}

impl From<TryFromIntError> for BakingPriorityError {
    fn from(_error: TryFromIntError) -> Self {
        Self::Overflow
    }
}

// ====================

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, thiserror::Error)]
pub enum BakingRightsError {
    #[error("Cannot encode block header: {0}")]
    Encode(String),
    #[error("Cannot create hash: {0}")]
    Hash(#[from] FromBytesError),
    #[error("Cryptography error: {0}")]
    Crypto(String),
}

impl From<BinaryWriterError> for BakingRightsError {
    fn from(error: BinaryWriterError) -> Self {
        Self::Encode(error.to_string())
    }
}

impl From<CryptoError> for BakingRightsError {
    fn from(error: CryptoError) -> Self {
        Self::Crypto(error.to_string())
    }
}
