// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::cmp::Ordering;
use std::collections::HashSet;
use std::sync::PoisonError;

use failure::Fail;

use crypto::hash::BlockHash;
use storage::StorageError;
use tezos_messages::p2p::encoding::prelude::OperationsForBlock;

pub mod block_state;
pub mod bootstrap_state;
pub mod head_state;
pub mod peer_state;
pub mod synchronization_state;

/// Possible errors for state processing
#[derive(Debug, Fail)]
pub enum StateError {
    #[fail(display = "Storage read/write error! Reason: {:?}", error)]
    StorageError { error: StorageError },
    #[fail(display = "Mutex/lock lock error! Reason: {:?}", reason)]
    LockError { reason: String },
    #[fail(display = "Processing error! Reason: {:?}", reason)]
    ProcessingError { reason: String },
}

impl<T> From<PoisonError<T>> for StateError {
    fn from(pe: PoisonError<T>) -> Self {
        StateError::LockError {
            reason: format!("{}", pe),
        }
    }
}

impl From<StorageError> for StateError {
    fn from(error: StorageError) -> Self {
        StateError::StorageError { error }
    }
}

impl From<failure::Error> for StateError {
    fn from(error: failure::Error) -> Self {
        StateError::ProcessingError {
            reason: format!("{}", error),
        }
    }
}

/// HistoryOrderPriority reflects order in history,
/// we want to download oldest blocks at first,
/// we use this atribute to prioritize
/// lowest value means as-soon-as-possible
pub type HistoryOrderPriority = usize;

// TODO: TE-386 - remove not needed
// pub struct MissingBlock {
//     pub block_hash: Arc<BlockHash>,
//     pub history_order_priority: HistoryOrderPriority,
// }
//
// impl BlockData for MissingBlock {
//     #[inline]
//     fn block_hash(&self) -> &BlockHash {
//         &self.block_hash
//     }
// }
//
// impl MissingBlock {
// TODO: TE-386 - remove not needed
// pub fn with_history_order(
//     block_hash: Arc<BlockHash>,
//     history_order_priority: HistoryOrderPriority,
// ) -> Self {
//     MissingBlock {
//         block_hash,
//         history_order_priority,
//     }
// }
// }
//
// impl PartialEq for MissingBlock {
//     fn eq(&self, other: &Self) -> bool {
//         self.block_hash == other.block_hash
//     }
// }
//
// impl Eq for MissingBlock {}
//
// impl PartialOrd for MissingBlock {
//     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
//         Some(self.cmp(other))
//     }
// }
//
// impl Ord for MissingBlock {
//     fn cmp(&self, other: &Self) -> Ordering {
//         // reverse, because we want lower level at begining
//         self.history_order_priority
//             .cmp(&other.history_order_priority)
//             .reverse()
//     }
// }

#[derive(Clone)]
pub struct MissingOperations {
    pub(crate) block_hash: BlockHash,
    pub(crate) validation_passes: HashSet<i8>,
    // TODO: TE-386 - remove not needed
    pub(crate) history_order_priority: HistoryOrderPriority,
    /// ttl retries - in case nobody has header, e.g.: in case of reorg or whatever
    pub(crate) retries: u8,
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

// impl BlockData for MissingOperations {
//     #[inline]
//     fn block_hash(&self) -> &BlockHash {
//         &self.block_hash
//     }
// }

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

    use super::*;

    fn block(d: u8) -> BlockHash {
        [d; crypto::hash::HashType::BlockHash.size()]
            .to_vec()
            .try_into()
            .expect("Failed to create BlockHash")
    }

    // TODO: TE-386 - remove not needed
    // #[test]
    // fn test_missing_blocks_has_correct_ordering() {
    //     let mut heap = MissingBlockData::default();
    //
    //     // simulate header and predecesor
    //     heap.push_data(MissingBlock::with_history_order(Arc::new(block(1)), 10, 1));
    //     heap.push_data(MissingBlock::with_history_order(Arc::new(block(2)), 9, 1));
    //
    //     // simulate history
    //     heap.push_data(MissingBlock::with_history_order(Arc::new(block(3)), 4, 1));
    //     heap.push_data(MissingBlock::with_history_order(Arc::new(block(7)), 0, 1));
    //     heap.push_data(MissingBlock::with_history_order(Arc::new(block(5)), 2, 1));
    //     heap.push_data(MissingBlock::with_history_order(Arc::new(block(6)), 1, 1));
    //     heap.push_data(MissingBlock::with_history_order(Arc::new(block(4)), 3, 1));
    //
    //     // pop all from heap
    //     let ordered_hashes = heap
    //         .drain_missing_data(heap.missing_data_count(), |_| true)
    //         .into_iter()
    //         .map(|b| b.block_hash.as_ref().clone())
    //         .collect::<Vec<BlockHash>>();
    //
    //     // ordered by priority: 0, 1, 2, 3, 4, 9, 10
    //     let expected_order: Vec<BlockHash> = vec![
    //         block(7),
    //         block(6),
    //         block(5),
    //         block(4),
    //         block(3),
    //         block(2),
    //         block(1),
    //     ];
    //
    //     assert_eq!(expected_order, ordered_hashes)
    // }

    // #[test]
    // fn missing_operation_has_correct_ordering() {
    //     let mut heap = MissingBlockData::default();
    //     heap.push_data(MissingOperations {
    //         history_order_priority: 15,
    //         block_hash: block(1),
    //         validation_passes: HashSet::new(),
    //         retries: 1,
    //     });
    //     heap.push_data(MissingOperations {
    //         history_order_priority: 7,
    //         block_hash: block(9),
    //         validation_passes: HashSet::new(),
    //         retries: 1,
    //     });
    //     heap.push_data(MissingOperations {
    //         history_order_priority: 0,
    //         block_hash: block(4),
    //         validation_passes: HashSet::new(),
    //         retries: 1,
    //     });
    //     heap.push_data(MissingOperations {
    //         history_order_priority: 1,
    //         block_hash: block(5),
    //         validation_passes: HashSet::new(),
    //         retries: 1,
    //     });
    //
    //     // TODO: TE-386 - remove not needed
    //     // pop all from heap
    //     let history_order_priorities = heap
    //         .drain_missing_data(4, |_| true)
    //         .into_iter()
    //         .map(|b| b.history_order_priority)
    //         .collect::<Vec<HistoryOrderPriority>>();
    //
    //     assert_eq!(vec![0, 1, 7, 15], history_order_priorities)
    // }

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
