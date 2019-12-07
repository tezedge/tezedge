// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;
use serde::{Deserialize, Serialize};

use storage::persistent::{Codec, DBError, Decoder, Encoder, SchemaError};

use crate::LEVEL_BASE;

/// Structure for orientation in the list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeHeader {
    /// Skip list ID
    list_id: SkipListId,
    /// Level on which this node exists
    lane_level: usize,
    /// Position of node in lane
    node_index: usize,
}

impl NodeHeader {
    pub fn new(list_id: SkipListId, lane_level: usize, node_index: usize) -> Self {
        Self {
            list_id,
            lane_level,
            node_index
        }
    }

    pub fn next(&self) -> Self {
        Self {
            list_id: self.list_id,
            lane_level: self.lane_level,
            node_index: self.node_index + 1,
        }
    }

    pub fn lower(&self) -> Self {
        if self.lane_level == 0 {
            self.clone()
        } else {
            Self {
                list_id: self.list_id,
                lane_level: self.lane_level - 1,
                node_index: self.lower_index(),
            }
        }
    }

    pub fn base_index(&self) -> usize {
        if self.lane_level == 0 {
            self.node_index
        } else {
            ((self.node_index + 1) * LEVEL_BASE.pow(self.lane_level as u32)) - 1
        }
    }

    pub fn lower_index(&self) -> usize {
        if self.lane_level == 0 {
            self.node_index
        } else {
            ((self.node_index + 1) * LEVEL_BASE) - 1
        }
    }

    pub fn level(&self) -> usize {
        self.lane_level
    }

    pub fn index(&self) -> usize {
        self.node_index
    }
}

impl Decoder for NodeHeader {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        bincode::deserialize(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }
}

impl Encoder for NodeHeader {
    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        bincode::serialize(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}

/// Trait representing node value.
/// Value must be able to be recreated by merging, and then split by difference.
/// diff = A.diff(B);
/// A.merge(diff) == B
pub trait ListValue: Codec + Default + std::fmt::Debug {
    /// Merge two values into one, in-place
    fn merge(&mut self, other: &Self);
    /// Create difference between two values.
    fn diff(&mut self, other: &Self);
}

/// ID of the skip list
pub type SkipListId = u16;

#[derive(Debug, Fail)]
pub enum SkipListError {
    #[fail(display = "Persistent storage error: {}", error)]
    PersistentStorageError {
        error: DBError
    }
}

impl From<DBError> for SkipListError {
    fn from(error: DBError) -> Self {
        SkipListError::PersistentStorageError { error }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn header_next() {
        let original = NodeHeader::new(0, 0, 0);
        let next = original.next();
        assert_eq!(original.lane_level, next.lane_level);
        assert_eq!(original.node_index + 1, next.node_index);
    }

    #[test]
    pub fn header_level() {
        let original = NodeHeader::new(0, 0, 0);
        assert_eq!(original.level(), 0);
    }

    #[test]
    pub fn header_index() {
        let original = NodeHeader::new(0, 0, 0);
        assert_eq!(original.index(), 0);
    }

    pub fn header_base_index() {
        let original = NodeHeader::new(0, 0, 0);
        assert_eq!(original.base_index(), 0);
        let original = NodeHeader::new(0, 1, 0);
        assert_eq!(original.base_index(), 7);
        let original = NodeHeader::new(0, 2, 0);
        assert_eq!(original.base_index(), 63);
    }

    pub fn header_lower_index() {
        let original = NodeHeader::new(0, 0, 0);
        assert_eq!(original.lower_index(), 0);
        let original = NodeHeader::new(0, 1, 0);
        assert_eq!(original.lower_index(), 7);
        let original = NodeHeader::new(0, 2, 0);
        assert_eq!(original.lower_index(), 7);
    }
}