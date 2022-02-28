// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::{TryFrom, TryInto};

use crypto::hash::{ChainId, HashTrait, Signature};
use tezos_messages::{
    base::signature_public_key::{SignaturePublicKey, SignatureWatermark},
    p2p::{binary_message::BinaryWrite, encoding::block_header::BlockHeader},
};

use crate::{current_head::CurrentHeadPrecheckError, Action};

use super::{
    current_head_actions::{
        CurrentHeadApplyAction, CurrentHeadPrecheckAction, CurrentHeadReceivedAction,
    },
    AppliedHead, BakingPriorityError, BakingRightsError, CurrentHeadState,
};

pub(super) const TIME_BETWEEN_BLOCKS: (i64, i64) = (20, 15);
pub(super) const MINIMAL_BLOCK_TIME: i64 = 15;

pub fn current_head_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    match &action.action {
        Action::CurrentHeadReceived(CurrentHeadReceivedAction {
            block_hash,
            block_header,
            ..
        }) => {
            let candidates = &mut state.current_heads.candidates;
            candidates
                .entry(block_hash.clone())
                .or_insert(super::CurrentHeadState::Received {
                    block_header: block_header.clone(),
                });
        }
        Action::CurrentHeadPrecheck(CurrentHeadPrecheckAction { block_hash, .. }) => {
            let chain_id = &state.config.chain_id;
            let baking_cache = &state.rights.cache.baking;
            let applied_head = match state.current_heads.applied_head() {
                Some(v) => v,
                None => return,
            };
            let applied_level = applied_head.level;
            let applied_timestamp = applied_head.timestamp;
            let candidates = &mut state.current_heads.candidates;
            candidates.get_mut(block_hash).map(|current_head_state| {
                if let CurrentHeadState::Received { block_header } = current_head_state {

                    let max_priority = match max_priority_for_prechecking(applied_timestamp, block_header.timestamp(), MINIMAL_BLOCK_TIME, TIME_BETWEEN_BLOCKS, action.duration_since_epoch().as_secs()) {
                        Ok(v) => v,
                        Err(err) => {
                            *current_head_state = CurrentHeadState::Error { error: err.into() };
                            return;
                        }
                    };

                    let priorities = if let Some((_, baking_rights)) = baking_cache
                        .get(&(applied_level + 1))
                    {
                        &baking_rights.priorities
                    } else {
                        *current_head_state = CurrentHeadState::Error { error: CurrentHeadPrecheckError::Other(format!("No precached baking rights for level `{level}`", level = block_header.level())) };
                        return;
                    };

                    if priorities.len() <= max_priority.into() {
                        *current_head_state = CurrentHeadState::Error { error: CurrentHeadPrecheckError::Other(format!("Not enough precached priorities, `{available}` available out of `{max_priority}`", available = priorities.len())) };
                        return;
                    }

                    *current_head_state = match precheck_block_header(block_header, chain_id, &priorities[..=(max_priority as usize)]) {
                        Ok(Some((delegate, priority))) => CurrentHeadState::Prechecked { block_header: block_header.clone(), baker: delegate.clone(), priority },
                        Ok(None) => CurrentHeadState::Rejected,
                        Err(err) => CurrentHeadState::Error { error: err.into() },
                    };
                }
            });
        }
        Action::CurrentHeadApply(CurrentHeadApplyAction {
            block_hash,
            level,
            timestamp,
        }) => {
            state.current_heads.applied_heads.push(AppliedHead {
                block_hash: block_hash.clone(),
                level: *level,
                timestamp: *timestamp,
            });
            state
                .current_heads
                .applied_hashes
                .insert(block_hash.clone(), *level);
            state.current_heads.candidates.clear();
        }
        _ => (),
    }
}

fn max_priority_for_prechecking(
    prev_timestamp: i64,
    timestamp: i64,
    minimal_time: i64,
    block_times: (i64, i64),
    now: u64,
) -> Result<u16, BakingPriorityError> {
    if timestamp > now.try_into()? {
        return Err(BakingPriorityError::TimeInFuture { now, timestamp });
    }
    if timestamp <= prev_timestamp {
        return Err(BakingPriorityError::TimeInPast {
            prev_timestamp,
            timestamp,
        });
    }
    if timestamp < prev_timestamp + minimal_time {
        return Err(BakingPriorityError::TooEarly {
            timestamp,
            prev_timestamp,
            min_timestamp: prev_timestamp + minimal_time,
        });
    }
    Ok(if timestamp < prev_timestamp + block_times.0 {
        0
    } else {
        ((timestamp - prev_timestamp - block_times.0) / block_times.1 + 1).try_into()?
    })
}

fn precheck_block_header(
    block_header: &BlockHeader,
    chain_id: &ChainId,
    priorities: &[SignaturePublicKey],
) -> Result<Option<(SignaturePublicKey, u16)>, BakingRightsError> {
    let encoded = block_header.as_bytes()?;
    let (unsigned, signature) =
        encoded.split_at(encoded.len().saturating_sub(Signature::hash_size()));
    let signature = Signature::try_from(signature)?;
    let watermark = SignatureWatermark::BlockHeader(chain_id.clone());
    for (priority, delegate) in priorities.iter().enumerate() {
        if delegate.verify_signature(&signature, &watermark, unsigned)? {
            return Ok(Some((delegate.clone(), priority as u16)));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_max_priority_for_prechecking_in_past() {
        let res =
            max_priority_for_prechecking(100, 99, MINIMAL_BLOCK_TIME, TIME_BETWEEN_BLOCKS, 101);
        assert!(matches!(res, Err(BakingPriorityError::TimeInPast { .. })));
    }

    #[test]
    fn test_max_priority_for_prechecking_in_future() {
        let res =
            max_priority_for_prechecking(100, 120, MINIMAL_BLOCK_TIME, TIME_BETWEEN_BLOCKS, 115);
        assert!(matches!(res, Err(BakingPriorityError::TimeInFuture { .. })));
    }

    #[test]
    fn test_max_priority_for_prechecking_too_early() {
        let res =
            max_priority_for_prechecking(100, 114, MINIMAL_BLOCK_TIME, TIME_BETWEEN_BLOCKS, 120);
        assert!(matches!(res, Err(BakingPriorityError::TooEarly { .. })));
    }

    #[test]
    fn test_max_priority_for_prechecking_0() {
        let res = max_priority_for_prechecking(
            100,
            100 + MINIMAL_BLOCK_TIME,
            MINIMAL_BLOCK_TIME,
            TIME_BETWEEN_BLOCKS,
            120,
        );
        assert_eq!(res, Ok(0));
    }

    #[test]
    fn test_max_priority_for_prechecking_1() {
        let res = max_priority_for_prechecking(
            100,
            100 + TIME_BETWEEN_BLOCKS.0,
            MINIMAL_BLOCK_TIME,
            TIME_BETWEEN_BLOCKS,
            (100 + TIME_BETWEEN_BLOCKS.0 + 5) as u64,
        );
        assert_eq!(res, Ok(1));
    }

    #[test]
    fn test_max_priority_for_prechecking_n() {
        const N: u16 = 10;

        let prev_timestamp = 100;
        let timestamp =
            prev_timestamp + TIME_BETWEEN_BLOCKS.0 + TIME_BETWEEN_BLOCKS.1 * (N as i64) - 1;
        let now = (timestamp + 1) as u64;
        let res = max_priority_for_prechecking(
            prev_timestamp,
            timestamp,
            MINIMAL_BLOCK_TIME,
            TIME_BETWEEN_BLOCKS,
            now,
        );
        assert_eq!(res, Ok(N));

        let timestamp = prev_timestamp + TIME_BETWEEN_BLOCKS.0 + TIME_BETWEEN_BLOCKS.1 * (N as i64);
        let now = (timestamp + 1) as u64;
        let res = max_priority_for_prechecking(
            prev_timestamp,
            timestamp,
            MINIMAL_BLOCK_TIME,
            TIME_BETWEEN_BLOCKS,
            now,
        );
        assert_eq!(res, Ok(N + 1));
    }
}
