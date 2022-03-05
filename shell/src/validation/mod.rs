// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Contains various validation functions:
//! - to support to validate different parts of the chain
//! - to ensure consistency of chain
//! - to support multipass validation

use std::time::Duration;

use tezos_protocol_ipc_client::{ProtocolRunnerConnection, ProtocolServiceError};
use thiserror::Error;

use crypto::hash::{BlockHash, ChainId, ProtocolHash};
use storage::block_meta_storage::Meta;
use storage::{BlockHeaderWithHash, StorageError};
use tezos_api::ffi::BeginApplicationRequest;
use tezos_messages::base::fitness_comparator::*;
use tezos_messages::p2p::binary_message::MessageHash;
use tezos_messages::p2p::encoding::fitness::Fitness;
use tezos_messages::p2p::encoding::prelude::BlockHeader;
use tezos_messages::{Head, TimestampOutOfRangeError};
use time::OffsetDateTime;

/// Validates if new_head is stronger or at least equals to old_head - according to fitness
pub fn can_update_current_head(
    new_head: &BlockHeaderWithHash,
    current_head: &Head,
    current_context_fitness: &Fitness,
) -> bool {
    let new_head_fitness = new_head.header.fitness();
    let current_head_fitness = current_head.fitness();
    let context_fitness = current_context_fitness;

    // according to chain_validator.ml
    if context_fitness.eq(&current_head_fitness) {
        new_head_fitness.gt(&context_fitness)
    } else {
        new_head_fitness.ge(&context_fitness)
    }
}

/// Returns only true, if new_fitness is greater than head's fitness
pub fn is_fitness_increases(head: &Head, new_fitness: &Fitness) -> bool {
    fitness_increases(head.fitness(), new_fitness)
}

/// Returns only true, if new_fitness is greater than head's fitness
pub fn is_fitness_increases_or_same(head: &Head, new_fitness: &Fitness) -> bool {
    fitness_increases_or_same(head.fitness(), new_fitness)
}

/// Returns true only if we recieve the same head as is our current_head
pub fn is_same_head(head: &Head, incoming_header: &BlockHeader) -> Result<bool, anyhow::Error> {
    let mut is_same = head.block_hash().as_ref() == &incoming_header.message_hash()?;
    is_same &= head.fitness() == incoming_header.fitness();
    is_same &= head.level() == &incoming_header.level();
    Ok(is_same)
}

/// Returns only true, if timestamp of header is not in the far future
pub fn is_future_block(block_header: &BlockHeader) -> Result<bool, anyhow::Error> {
    let future_margin = OffsetDateTime::now_utc() + Duration::from_secs(15);
    let block_timestamp = OffsetDateTime::from_unix_timestamp(block_header.timestamp())
        .map_err(|_| TimestampOutOfRangeError)?;
    Ok(block_timestamp > future_margin)
}

pub enum CanApplyStatus {
    Ready,
    AlreadyApplied,
    MissingPredecessor,
    PredecessorNotApplied,
    MissingOperations,
}

/// Returns true, if [block] can be applied
pub fn can_apply_block<'b, OP, PA>(
    (block, block_metadata): (&'b BlockHash, &'b Meta),
    operations_complete: OP,
    predecessor_applied: PA,
) -> Result<CanApplyStatus, StorageError>
where
    OP: Fn(&'b BlockHash) -> Result<bool, StorageError>, /* func returns true, if operations are completed */
    PA: Fn(&'b BlockHash) -> Result<bool, StorageError>, /* func returns true, if predecessor is applied */
{
    let block_predecessor = block_metadata.predecessor();

    // check if block is already applied, dont need to apply second time
    if block_metadata.is_applied() {
        return Ok(CanApplyStatus::AlreadyApplied);
    }

    // we need to have predecessor (every block has)
    if block_predecessor.is_none() {
        return Ok(CanApplyStatus::MissingPredecessor);
    }

    // if operations are not complete, we cannot apply block
    if !operations_complete(block)? {
        return Ok(CanApplyStatus::MissingOperations);
    }

    // check if predecesor is applied
    if let Some(predecessor) = block_predecessor {
        if predecessor_applied(predecessor)? {
            return Ok(CanApplyStatus::Ready);
        }
    }

    Ok(CanApplyStatus::PredecessorNotApplied)
}

/// Error produced by a [prevalidate_operation].
#[derive(Debug, Error)]
pub enum PrevalidateOperationError {
    #[error("Unknown branch ({branch}), cannot inject the operation.")]
    UnknownBranch { branch: String },
    #[error("Branch is not applied yet ({branch}), cannot inject the operation.")]
    BranchNotAppliedYet { branch: String },
    #[error("Prevalidator is not running ({reason}), cannot inject the operation.")]
    PrevalidatorNotInitialized { reason: String },
    #[error("Storage read error, reason: {error:?}")]
    StorageError { error: StorageError },
    #[error("Failed to prevalidate operation: {operation_hash}, reason: {reason:?}")]
    ValidationError {
        operation_hash: String,
        reason: ProtocolServiceError,
    },
    #[error("Operation ({operation_hash}) is already in mempool, cannot inject the operation.")]
    AlreadyInMempool { operation_hash: String },
    #[error("Failed to prevalidate operation ({operation_hash}), cannot inject the operation, reason: {reason}")]
    UnexpectedError {
        operation_hash: String,
        reason: String,
    },
}

impl From<StorageError> for PrevalidateOperationError {
    fn from(error: StorageError) -> Self {
        PrevalidateOperationError::StorageError { error }
    }
}

/// Implementation for multipass validation:
/// - checks encoding for protocol_data
/// - checks begin_application, if predecessor
///
/// Returns None if everything, else return error
pub async fn check_multipass_validation(
    chain_id: &ChainId,
    protocol_hash: ProtocolHash,
    validated_block_header: &BlockHeader,
    predecessor: Option<BlockHeaderWithHash>,
    api: &mut ProtocolRunnerConnection,
) -> Option<ProtocolServiceError> {
    // 1. check encoding for protocol_data
    if let Err(e) = api
        .assert_encoding_for_protocol_data(
            protocol_hash,
            validated_block_header.protocol_data().clone(),
        )
        .await
    {
        return Some(e);
    };

    // 2. lets check strict multipasss validation with protocol for block_header
    if let Some(predecessor) = predecessor {
        let request = BeginApplicationRequest {
            chain_id: chain_id.clone(),
            pred_header: predecessor.header.as_ref().clone(),
            block_header: validated_block_header.clone(),
        };

        if let Err(e) = api.begin_application(request).await {
            return Some(e);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::{convert::TryInto, sync::Arc};

    use tezos_messages::p2p::encoding::fitness::Fitness;
    use tezos_messages::p2p::encoding::prelude::BlockHeaderBuilder;

    use super::*;

    macro_rules! fitness {
        ( $($x:expr),* ) => {{
            Fitness::from(vec![
                $(
                    $x.to_vec(),
                )*
            ])
        }}
    }

    #[test]
    fn test_can_update_current_head() -> Result<(), anyhow::Error> {
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

    #[test]
    fn is_future_block_panics_on_bad_timeout() {
        let block_header = BlockHeaderBuilder::default()
            .level(34)
            .proto(1)
            .predecessor(
                "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET"
                    .try_into()
                    .unwrap(),
            )
            .timestamp(-3551937681785568940)
            .validation_pass(4)
            .operations_hash(
                "LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc"
                    .try_into()
                    .unwrap(),
            )
            .fitness(fitness!([0], [0, 0, 1]))
            .context(
                "CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd"
                    .try_into()
                    .unwrap(),
            )
            .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
            .build()
            .unwrap();
        assert!(is_future_block(&block_header).is_err());
    }

    fn new_head(fitness: Fitness) -> Result<BlockHeaderWithHash, anyhow::Error> {
        Ok(BlockHeaderWithHash {
            hash: "BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET".try_into()?,
            header: Arc::new(
                BlockHeaderBuilder::default()
                    .level(34)
                    .proto(1)
                    .predecessor("BKyQ9EofHrgaZKENioHyP4FZNsTmiSEcVmcghgzCC9cGhE7oCET".try_into()?)
                    .timestamp(5_635_634)
                    .validation_pass(4)
                    .operations_hash(
                        "LLoaGLRPRx3Zf8kB4ACtgku8F4feeBiskeb41J1ciwfcXB3KzHKXc".try_into()?,
                    )
                    .fitness(fitness)
                    .context("CoVmAcMV64uAQo8XvfLr9VDuz7HVZLT4cgK1w1qYmTjQNbGwQwDd".try_into()?)
                    .protocol_data(vec![0, 1, 2, 3, 4, 5, 6, 7, 8])
                    .build()
                    .unwrap(),
            ),
        })
    }

    fn current_head(fitness: Fitness) -> Result<Head, anyhow::Error> {
        Ok(Head::new(
            "BKzyxvaMgoY5M3BUD7UaUCPivAku2NRiYRA1z1LQUzB7CX6e8yy".try_into()?,
            5,
            fitness,
        ))
    }
}
