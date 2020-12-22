// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Contains various validation functions:
//! - to support to validate different parts of the chain
//! - to ensure consistency of chain
//! - to support multipass validation

use std::time::Duration;

use chrono::TimeZone;
use failure::Fail;

use crypto::hash::{ChainId, HashType, OperationHash, ProtocolHash};
use storage::{BlockHeaderWithHash, BlockMetaStorageReader, BlockStorageReader, StorageError};
use tezos_api::ffi::{
    BeginApplicationRequest, BeginConstructionRequest, ValidateOperationRequest,
    ValidateOperationResult,
};
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::block_header::Fitness;
use tezos_messages::p2p::encoding::prelude::{BlockHeader, Operation};
use tezos_messages::Head;
use tezos_wrapper::service::{ProtocolController, ProtocolServiceError};

use crate::mempool::CurrentMempoolStateStorageRef;
use crate::validation::fitness_comparator::FitnessWrapper;

/// Validates if new_head is stronger or at least equals to old_head - according to fitness
pub fn can_update_current_head(
    new_head: &BlockHeaderWithHash,
    current_head: &Head,
    current_context_fitness: &Fitness,
) -> bool {
    let new_head_fitness = FitnessWrapper::new(new_head.header.fitness());
    let current_head_fitness = FitnessWrapper::new(current_head.fitness());
    let context_fitness = FitnessWrapper::new(current_context_fitness);

    // according to chain_validator.ml
    if context_fitness.eq(&current_head_fitness) {
        new_head_fitness.gt(&current_head_fitness)
    } else {
        new_head_fitness.ge(&context_fitness)
    }
}

/// Returns only true, if new_fitness is greater than head's fitness
pub fn is_fitness_increases(head: &Head, new_fitness: &Fitness) -> bool {
    new_fitness.gt(head.fitness())
}

/// Returns only true, if new_fitness is greater than head's fitness
pub fn is_fitness_increases_or_same(head: &Head, new_fitness: &Fitness) -> bool {
    new_fitness.ge(head.fitness())
}

/// Returns true only if we recieve the same head as is our current_head
pub fn is_same_head(head: &Head, incoming_header: &BlockHeader) -> Result<bool, failure::Error> {
    let mut is_same = head.block_hash() == &incoming_header.message_hash()?;
    is_same &= head.fitness() == incoming_header.fitness();
    is_same &= head.level() == &incoming_header.level();
    Ok(is_same)
}

/// Returns only true, if timestamp of header is not in the far future
pub fn is_future_block(block_header: &BlockHeader) -> Result<bool, failure::Error> {
    let future_margin =
        chrono::offset::Utc::now() + chrono::Duration::from_std(Duration::from_secs(15))?;
    let block_timestamp = chrono::Utc.from_utc_datetime(&chrono::NaiveDateTime::from_timestamp(
        block_header.timestamp(),
        0,
    ));
    Ok(block_timestamp > future_margin)
}

/// Returns true, if we can accept injected operation from rpc
pub fn can_accept_operation_from_rpc(
    operation_hash: &OperationHash,
    result: &ValidateOperationResult,
) -> bool {
    // we can accept from rpc, only if it is [applied]
    result
        .applied
        .iter()
        .any(|operation_result| operation_result.hash.eq(operation_hash))
}

/// Returns true, if we can accept received operation from p2p
pub fn can_accept_operation_from_p2p(
    operation_hash: &OperationHash,
    result: &ValidateOperationResult,
) -> bool {
    // we can accept from p2p, only if it is [not refused]
    if result
        .refused
        .iter()
        .any(|operation_result| operation_result.hash.eq(operation_hash))
    {
        return false;
    }

    // true, if contained in applied
    if result
        .applied
        .iter()
        .any(|operation_result| operation_result.hash.eq(operation_hash))
    {
        return true;
    }

    // true, if contained in branch_refused
    if result
        .branch_refused
        .iter()
        .any(|operation_result| operation_result.hash.eq(operation_hash))
    {
        return true;
    }

    // true, if contained in branch_refused
    if result
        .branch_delayed
        .iter()
        .any(|operation_result| operation_result.hash.eq(operation_hash))
    {
        return true;
    }

    // any just false
    false
}

/// Error produced by a [prevalidate_operation].
#[derive(Debug, Fail)]
pub enum PrevalidateOperationError {
    #[fail(display = "Unknown branch ({}), cannot inject the operation.", branch)]
    UnknownBranch { branch: String },
    #[fail(
        display = "Branch is not applied yet ({}), cannot inject the operation.",
        branch
    )]
    BranchNotAppliedYet { branch: String },
    #[fail(
        display = "Prevalidator is not running ({}), cannot inject the operation.",
        reason
    )]
    PrevalidatorNotInitialized { reason: String },
    #[fail(display = "Storage read error! Reason: {:?}", error)]
    StorageError { error: StorageError },
    #[fail(
        display = "Failed to validate operation: {}! Reason: {:?}",
        operation_hash, reason
    )]
    ValidationError {
        operation_hash: String,
        reason: ProtocolServiceError,
    },
}

impl From<StorageError> for PrevalidateOperationError {
    fn from(error: StorageError) -> Self {
        PrevalidateOperationError::StorageError { error }
    }
}

/// Validates operation before added to mempool
/// Operation is decoded and applied to context according to current head in mempool
pub fn prevalidate_operation(
    chain_id: &ChainId,
    operation_hash: &OperationHash,
    operation: &Operation,
    current_mempool_state: CurrentMempoolStateStorageRef,
    api: &ProtocolController,
    block_storage: &Box<dyn BlockStorageReader>,
    block_meta_storage: &Box<dyn BlockMetaStorageReader>,
) -> Result<ValidateOperationResult, PrevalidateOperationError> {
    // just check if we know block from operation (and is applied)
    let operation_branch = operation.branch();
    match block_storage.contains(operation_branch)? {
        true => {
            let is_applied = match block_meta_storage.get(operation_branch)? {
                Some(metadata) => metadata.is_applied(),
                None => false,
            };

            if !is_applied {
                return Err(PrevalidateOperationError::BranchNotAppliedYet {
                    branch: HashType::BlockHash.hash_to_b58check(&operation_branch),
                });
            }
        }
        false => {
            return Err(PrevalidateOperationError::UnknownBranch {
                branch: HashType::BlockHash.hash_to_b58check(&operation_branch),
            })
        }
    }

    // get actual known state of mempool, we need the same head as used actualy be mempool
    let mempool_state = current_mempool_state.read().unwrap();
    let mempool_head = match mempool_state.head().as_ref() {
        Some(head) => match block_storage.get(head)? {
            Some(head) => head,
            None => {
                return Err(PrevalidateOperationError::UnknownBranch {
                    branch: HashType::BlockHash.hash_to_b58check(&head),
                })
            }
        },
        None => {
            return Err(PrevalidateOperationError::PrevalidatorNotInitialized {
                reason: "no head in mempool".to_string(),
            });
        }
    };

    // TODO: possible to add ffi to pre_filter
    // TODO: possible to add ffi to decode_operation_data

    // begin construction of a new empty block
    let prevalidator = api
        .begin_construction(BeginConstructionRequest {
            chain_id: chain_id.clone(),
            predecessor: (&*mempool_head.header).clone(),
            protocol_data: None,
        })
        .map_err(|e| PrevalidateOperationError::ValidationError {
            operation_hash: HashType::OperationHash.hash_to_b58check(operation_hash),
            reason: e,
        })?;

    // validate operation to new empty/dummpy block
    api.validate_operation(ValidateOperationRequest {
        prevalidator,
        operation: operation.clone(),
    })
    .map(|r| r.result)
    .map_err(|e| PrevalidateOperationError::ValidationError {
        operation_hash: HashType::OperationHash.hash_to_b58check(operation_hash),
        reason: e,
    })
}

/// Implementation for multipass validation:
/// - checks encoding for protocol_data
/// - checks begin_application, if predecessor
///
/// Returns None if everything, else return error
pub fn check_multipass_validation(
    chain_id: &ChainId,
    protocol_hash: ProtocolHash,
    validated_block_header: &BlockHeader,
    predecessor: Option<BlockHeaderWithHash>,
    api: &ProtocolController,
) -> Option<ProtocolServiceError> {
    // 1. check encoding for protocol_data
    if let Err(e) = api.assert_encoding_for_protocol_data(
        protocol_hash,
        validated_block_header.protocol_data().clone(),
    ) {
        return Some(e);
    };

    // 2. lets check strict multipasss validation with protocol for block_header
    if let Some(predecessor) = predecessor {
        let request = BeginApplicationRequest {
            chain_id: chain_id.clone(),
            pred_header: predecessor.header.as_ref().clone(),
            block_header: validated_block_header.clone(),
        };

        if let Err(e) = api.begin_application(request) {
            return Some(e);
        }
    }

    None
}

/// Fitness comparison:
///     - shortest lists are smaller
///     - lexicographical order for lists of the same length.
pub mod fitness_comparator {
    use failure::_core::cmp::Ordering;

    use tezos_messages::p2p::encoding::block_header::Fitness;

    pub struct FitnessWrapper<'a> {
        fitness: &'a Fitness,
    }

    impl<'a> FitnessWrapper<'a> {
        pub fn new(fitness: &'a Fitness) -> Self {
            FitnessWrapper { fitness }
        }
    }

    impl<'a> PartialOrd for FitnessWrapper<'a> {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl<'a> Ord for FitnessWrapper<'a> {
        fn cmp(&self, other: &Self) -> Ordering {
            // length of fitness list must be equal
            let result = self.fitness.len().cmp(&other.fitness.len());
            if result != Ordering::Equal {
                return result;
            }

            // if length is same, we need to compare by elements
            let fitness_count = self.fitness.len();
            for i in 0..fitness_count {
                let self_fitness_part = &self.fitness[i];
                let other_fitness_part = &other.fitness[i];

                // length of fitness must be equal
                let result = self_fitness_part.len().cmp(&other_fitness_part.len());
                if result != Ordering::Equal {
                    return result;
                }

                // now compare by-bytes from left
                let part_length = self_fitness_part.len();
                for j in 0..part_length {
                    let b1 = self_fitness_part[j];
                    let b2 = other_fitness_part[j];
                    let byte_result = b1.cmp(&b2);
                    if byte_result != Ordering::Equal {
                        return byte_result;
                    }
                }
            }

            Ordering::Equal
        }
    }

    impl<'a> Eq for FitnessWrapper<'a> {}

    impl<'a> PartialEq for FitnessWrapper<'a> {
        fn eq(&self, other: &Self) -> bool {
            self.fitness == other.fitness
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crypto::hash::HashType;
    use tezos_messages::p2p::encoding::block_header::Fitness;
    use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

    use super::*;

    macro_rules! fitness {
        ( $($x:expr),* ) => {{
            let fitness: Fitness = vec![
                $(
                    $x.to_vec(),
                )*
            ];
            fitness
        }}
    }

    #[test]
    fn test_can_update_current_head() -> Result<(), failure::Error> {
        assert_eq!(
            false,
            can_update_current_head(
                &new_head(fitness!([0]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 2]),
            )
        );
        assert_eq!(
            false,
            can_update_current_head(
                &new_head(fitness!([0], [0, 1]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 2]),
            )
        );
        assert_eq!(
            false,
            can_update_current_head(
                &new_head(fitness!([0], [0, 0, 1]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 2]),
            )
        );
        assert_eq!(
            false,
            can_update_current_head(
                &new_head(fitness!([0], [0, 0, 2]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 2]),
            )
        );
        assert_eq!(
            true,
            can_update_current_head(
                &new_head(fitness!([0], [0, 0, 3]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 2]),
            )
        );
        assert_eq!(
            true,
            can_update_current_head(
                &new_head(fitness!([0], [0, 0, 1], [0]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 2]),
            )
        );
        assert_eq!(
            true,
            can_update_current_head(
                &new_head(fitness!([0], [0, 0, 0, 1]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 2]),
            )
        );

        // context fitnes is lower than current head
        assert_eq!(
            true,
            can_update_current_head(
                &new_head(fitness!([0], [0, 0, 2]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 1]),
            )
        );
        // context fitnes is higher than current head
        assert_eq!(
            false,
            can_update_current_head(
                &new_head(fitness!([0], [0, 0, 2]))?,
                &current_head(fitness!([0], [0, 0, 2]))?,
                &fitness!([0], [0, 0, 3]),
            )
        );

        Ok(())
    }

    fn new_head(fitness: Fitness) -> Result<BlockHeaderWithHash, failure::Error> {
        Ok(BlockHeaderWithHash {
            hash: HashType::BlockHash
                .b58check_to_hash("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET")?,
            header: Arc::new(
                BlockHeaderBuilder::default()
                    .level(34)
                    .proto(1)
                    .predecessor(
                        HashType::BlockHash.b58check_to_hash(
                            "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET",
                        )?,
                    )
                    .timestamp(5_635_634)
                    .validation_pass(4)
                    .operations_hash(HashType::OperationListListHash.b58check_to_hash(
                        "LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc",
                    )?)
                    .fitness(fitness)
                    .context(
                        HashType::ContextHash.b58check_to_hash(
                            "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd",
                        )?,
                    )
                    .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
                    .build()
                    .unwrap(),
            ),
        })
    }

    fn current_head(fitness: Fitness) -> Result<Head, failure::Error> {
        Ok(Head::new(
            HashType::BlockHash
                .b58check_to_hash("BKzyxvaMgoY5M3BUD7UaUCPivAku2NRiYRA1z1LQUzB7CX6e8yy")?,
            5,
            fitness,
        ))
    }
}
