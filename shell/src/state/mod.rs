// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;
use std::collections::HashSet;

use crypto::hash::BlockHash;
use tezos_messages::p2p::encoding::prelude::OperationsForBlock;

use crate::utils::collections::BlockData;

pub mod block_state;
pub mod peer_state;

/// HistoryOrderPriority reflects order in history,
/// we want to download oldest blocks at first,
/// we use this atribute to prioritize
/// lowest value means as-soon-as-possible
pub type HistoryOrderPriority = usize;

pub struct MissingBlock {
    pub block_hash: BlockHash,
    pub history_order_priority: HistoryOrderPriority,

    /// ttl retries - in case nobody has header, e.g.: in case of reorg or whatever
    retries: u8,
}

impl BlockData for MissingBlock {
    #[inline]
    fn block_hash(&self) -> &BlockHash {
        &self.block_hash
    }
}

impl MissingBlock {
    pub fn with_history_order(
        block_hash: BlockHash,
        history_order_priority: HistoryOrderPriority,
        retries: u8,
    ) -> Self {
        MissingBlock {
            block_hash,
            history_order_priority,
            retries,
        }
    }

    /// Returns true, we can do another retry
    pub fn retry(&mut self) -> bool {
        if self.retries == 0 {
            return false;
        }
        self.retries -= 1;
        true
    }
}

impl PartialEq for MissingBlock {
    fn eq(&self, other: &Self) -> bool {
        self.block_hash == other.block_hash
    }
}

impl Eq for MissingBlock {}

impl PartialOrd for MissingBlock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MissingBlock {
    fn cmp(&self, other: &Self) -> Ordering {
        // reverse, because we want lower level at begining
        self.history_order_priority
            .cmp(&other.history_order_priority)
            .reverse()
    }
}

#[derive(Clone)]
pub struct MissingOperations {
    block_hash: BlockHash,
    pub(crate) validation_passes: HashSet<i8>,
    history_order_priority: HistoryOrderPriority,
    /// ttl retries - in case nobody has header, e.g.: in case of reorg or whatever
    retries: u8,
}

impl MissingOperations {
    /// Returns true, we can do another retry
    pub fn retry(&mut self) -> bool {
        if self.retries == 0 {
            return false;
        }
        self.retries -= 1;
        true
    }
}

impl BlockData for MissingOperations {
    #[inline]
    fn block_hash(&self) -> &BlockHash {
        &self.block_hash
    }
}

impl PartialEq for MissingOperations {
    fn eq(&self, other: &Self) -> bool {
        self.history_order_priority == other.history_order_priority
            && self.block_hash == other.block_hash
    }
}

impl Eq for MissingOperations {}

impl PartialOrd for MissingOperations {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MissingOperations {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.history_order_priority, &self.block_hash)
            .cmp(&(other.history_order_priority, &other.block_hash))
            .reverse()
    }
}

impl From<&MissingOperations> for Vec<OperationsForBlock> {
    fn from(ops: &MissingOperations) -> Self {
        ops.validation_passes
            .iter()
            .map(|vp| OperationsForBlock::new(ops.block_hash.clone(), *vp))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use crate::utils::MissingBlockData;

    use super::*;

    fn block(d: u8) -> BlockHash {
        [d; crypto::hash::HashType::BlockHash.size()]
            .to_vec()
            .try_into()
            .unwrap()
    }

    #[test]
    fn test_missing_blocks_has_correct_ordering() {
        let mut heap = MissingBlockData::default();

        // simulate header and predecesor
        heap.push_data(MissingBlock::with_history_order(block(1), 10, 1));
        heap.push_data(MissingBlock::with_history_order(block(2), 9, 1));

        // simulate history
        heap.push_data(MissingBlock::with_history_order(block(3), 4, 1));
        heap.push_data(MissingBlock::with_history_order(block(7), 0, 1));
        heap.push_data(MissingBlock::with_history_order(block(5), 2, 1));
        heap.push_data(MissingBlock::with_history_order(block(6), 1, 1));
        heap.push_data(MissingBlock::with_history_order(block(4), 3, 1));

        // pop all from heap
        let ordered_hashes = heap
            .drain_missing_data(heap.missing_data_count(), |_| true)
            .into_iter()
            .map(|b| b.block_hash)
            .collect::<Vec<BlockHash>>();

        // ordered by priority: 0, 1, 2, 3, 4, 9, 10
        let expected_order: Vec<BlockHash> = vec![
            block(7),
            block(6),
            block(5),
            block(4),
            block(3),
            block(2),
            block(1),
        ];

        assert_eq!(expected_order, ordered_hashes)
    }

    #[test]
    fn missing_operation_has_correct_ordering() {
        let mut heap = MissingBlockData::default();
        heap.push_data(MissingOperations {
            history_order_priority: 15,
            block_hash: block(1),
            validation_passes: HashSet::new(),
            retries: 1,
        });
        heap.push_data(MissingOperations {
            history_order_priority: 7,
            block_hash: block(9),
            validation_passes: HashSet::new(),
            retries: 1,
        });
        heap.push_data(MissingOperations {
            history_order_priority: 0,
            block_hash: block(4),
            validation_passes: HashSet::new(),
            retries: 1,
        });
        heap.push_data(MissingOperations {
            history_order_priority: 1,
            block_hash: block(5),
            validation_passes: HashSet::new(),
            retries: 1,
        });

        // pop all from heap
        let history_order_priorities = heap
            .drain_missing_data(heap.missing_data_count(), |_| true)
            .into_iter()
            .map(|b| b.history_order_priority)
            .collect::<Vec<HistoryOrderPriority>>();

        assert_eq!(vec![0, 1, 7, 15], history_order_priorities)
    }

    #[test]
    fn test_retry() {
        let mut data = MissingOperations {
            history_order_priority: 1,
            block_hash: block(5),
            validation_passes: HashSet::new(),
            retries: 5,
        };

        assert!(data.retry());
        assert!(data.retry());
        assert!(data.retry());
        assert!(data.retry());
        assert!(data.retry());
        assert!(!data.retry());
        assert!(!data.retry());
    }
}
