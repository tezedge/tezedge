// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod rights_state;
use std::{convert::TryFrom, num::ParseIntError};

use crypto::{base58::FromBase58CheckError, hash::BlockHash};
pub use rights_state::*;

pub mod rights_actions;

mod rights_reducer;
pub use rights_reducer::rights_reducer;

mod rights_effects;
pub use rights_effects::rights_effects;
use tezos_messages::p2p::encoding::block_header::Level;

mod utils;

/// Key identifying particular request for endorsing rights.
#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
#[serde(into = "String", try_from = "String")]
pub struct EndorsingRightsKey {
    /// Current block hash.
    pub current_block_hash: BlockHash,
    /// Level of block to calculate endorsing rights for. If `None`, `current_block_hash` is used instead.
    pub level: Option<Level>,
}

impl From<EndorsingRightsKey> for String {
    fn from(source: EndorsingRightsKey) -> Self {
        format!(
            "{}{}",
            source.current_block_hash.to_base58_check(),
            source
                .level
                .map(|level| format!(":{}", level.to_string()))
                .unwrap_or_default()
        )
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EndorsingRightsKeyFromStringError {
    #[error(transparent)]
    FromBase58(#[from] FromBase58CheckError),
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
}

impl TryFrom<String> for EndorsingRightsKey {
    type Error = EndorsingRightsKeyFromStringError;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        let (hash, level) = source
            .split_once(':')
            .map(|(a, b)| (a, Some(b)))
            .unwrap_or((&source, None));
        Ok(EndorsingRightsKey {
            current_block_hash: BlockHash::from_base58_check(hash)?,
            level: level.map(|level| level.parse()).transpose()?,
        })
    }
}

pub use crate::storage::kv_cycle_meta::Cycle;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crypto::hash::BlockHash;

    use super::EndorsingRightsKey;

    #[test]
    fn can_serde_json_hash_map() {
        let key = EndorsingRightsKey {
            current_block_hash: BlockHash::from_base58_check(
                "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe",
            )
            .unwrap(),
            level: Some(10),
        };
        let hash_map = HashMap::from([(key.clone(), true)]);
        let json = serde_json::to_string(&hash_map).unwrap();
        let deserialized: HashMap<_, _> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, hash_map);

        let key = EndorsingRightsKey {
            current_block_hash: BlockHash::from_base58_check(
                "BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe",
            )
            .unwrap(),
            level: None,
        };
        let hash_map = HashMap::from([(key.clone(), true)]);
        let json = serde_json::to_string(&hash_map).unwrap();
        let deserialized: HashMap<_, _> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, hash_map);
    }
}
