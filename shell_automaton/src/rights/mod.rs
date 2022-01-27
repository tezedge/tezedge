// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod rights_state;

use crypto::hash::BlockHash;
pub use rights_state::*;

pub mod rights_actions;

mod rights_reducer;
pub use rights_reducer::rights_reducer;

mod rights_effects;
pub use rights_effects::rights_effects;
use tezos_messages::p2p::encoding::block_header::Level;

mod utils;

/// Key identifying particular request for endorsing rights.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RightsKey(RightsInput);

impl RightsKey {
    pub(super) fn baking(
        current_block_hash: BlockHash,
        level: Option<Level>,
        _max_priority: Option<u16>,
    ) -> Self {
        Self(RightsInput::Baking(BakingRightsInput {
            current_block_hash,
            level,
        }))
    }
    pub(super) fn endorsing(current_block_hash: BlockHash, level: Option<Level>) -> Self {
        Self(RightsInput::Endorsing(EndorsingRightsInput {
            current_block_hash,
            level,
        }))
    }

    pub(super) fn block(&self) -> &BlockHash {
        match &self.0 {
            RightsInput::Baking(BakingRightsInput {
                current_block_hash, ..
            }) => current_block_hash,
            RightsInput::Endorsing(EndorsingRightsInput {
                current_block_hash, ..
            }) => current_block_hash,
        }
    }

    pub(super) fn level(&self) -> Option<Level> {
        match self.0 {
            RightsInput::Baking(BakingRightsInput { level, .. }) => level,
            RightsInput::Endorsing(EndorsingRightsInput { level, .. }) => level,
        }
    }
}

impl From<RightsKey> for RightsInput {
    fn from(key: RightsKey) -> Self {
        key.0
    }
}

impl serde::Serialize for RightsKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let v = serde_json::to_string(&self.0).map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&v)
    }
}

impl<'de> serde::Deserialize<'de> for RightsKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct HashVisitor;

        impl<'de> serde::de::Visitor<'de> for HashVisitor {
            type Value = RightsInput;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("eigher sequence of bytes or base58 encoded data expected")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                serde_json::from_str(v).map_err(|e| {
                    serde::de::Error::custom(format!(
                        "error constructing rights key from json: {}",
                        e
                    ))
                })
            }
        }

        deserializer.deserialize_str(HashVisitor).map(Self)
    }
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub enum RightsInput {
    Baking(BakingRightsInput),
    Endorsing(EndorsingRightsInput),
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct BakingRightsInput {
    /// Current block hash.
    pub current_block_hash: BlockHash,
    /// Level of block to calculate endorsing rights for. If `None`, `current_block_hash` is used instead.
    pub level: Option<Level>,
}

#[derive(
    Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct EndorsingRightsInput {
    /// Current block hash.
    pub current_block_hash: BlockHash,
    /// Level of block to calculate endorsing rights for. If `None`, `current_block_hash` is used instead.
    pub level: Option<Level>,
}

pub use crate::storage::kv_cycle_meta::Cycle;

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crypto::hash::BlockHash;

    use super::RightsKey;

    #[test]
    fn rights_key_serialize_deserialize() {
        let hash =
            BlockHash::from_base58_check("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")
                .unwrap();

        let key = RightsKey::endorsing(hash.clone(), Some(10));
        let s: String = serde_json::to_string(&key).unwrap();
        assert_eq!(key, serde_json::from_str(&s).unwrap());

        let key = RightsKey::baking(hash.clone(), None, None);
        let s: String = serde_json::to_string(&key).unwrap();
        assert_eq!(key, serde_json::from_str(&s).unwrap());
    }

    #[test]
    fn can_serde_json_hash_map() {
        let key = RightsKey::endorsing(
            BlockHash::from_base58_check("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")
                .unwrap(),
            Some(10),
        );
        let hash_map = HashMap::from([(key.clone(), true)]);
        let json = serde_json::to_string(&hash_map).unwrap();
        let deserialized: HashMap<_, _> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, hash_map);

        let key = RightsKey::endorsing(
            BlockHash::from_base58_check("BLockGenesisGenesisGenesisGenesisGenesisb83baZgbyZe")
                .unwrap(),
            None,
        );
        let hash_map = HashMap::from([(key.clone(), true)]);
        let json = serde_json::to_string(&hash_map).unwrap();
        let deserialized: HashMap<_, _> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, hash_map);
    }
}
