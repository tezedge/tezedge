use storage::persistent::{Codec, SchemaError};
use serde::{Serialize, Deserialize};

/// Structure for orientation in the list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeHeader {
    /// Level on which this node exists
    lane_level: usize,
    /// Position of node in lane
    node_index: usize,
}

impl NodeHeader {
    pub fn new(lane_level: usize, node_index: usize) -> Self {
        Self { lane_level, node_index }
    }

    pub fn next(&self) -> Self {
        Self {
            lane_level: self.lane_level,
            node_index: self.node_index + 1,
        }
    }
}

impl Codec for NodeHeader {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError> {
        bincode::deserialize(bytes)
            .map_err(|_| SchemaError::DecodeError)
    }

    fn encode(&self) -> Result<Vec<u8>, SchemaError> {
        bincode::serialize(self)
            .map_err(|_| SchemaError::EncodeError)
    }
}

/// Trait representing node value.
/// Value must be able to be recreated by merging, and then split by difference.
/// diff = A.diff(B);
/// A.merge(diff) == B
pub trait ListValue: Codec + Default {
    /// Merge two values into one, in-place
    fn merge(&mut self, other: &Self);
    /// Create difference between two values.
    fn diff(&mut self, other: &Self);
}