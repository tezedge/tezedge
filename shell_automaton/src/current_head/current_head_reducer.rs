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

pub fn current_head_reducer(state: &mut crate::State, action: &crate::ActionWithMeta) {
    match &action.action {
        Action::CurrentHeadReceived(CurrentHeadReceivedAction {
            address: _,
            block_hash,
            block_header,
        }) => {
            let candidates = &mut state.current_heads.candidates;
            candidates
                .entry(block_hash.clone())
                .or_insert(super::CurrentHeadState::Received {
                    block_header: block_header.clone(),
                });
        }
        Action::CurrentHeadPrecheck(CurrentHeadPrecheckAction { block_hash }) => {
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

                    let max_priority = match max_priority_for_prechecking(applied_timestamp, block_header.timestamp(), (20, 10), action.duration_since_epoch().as_secs()) {
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
                        Ok(Some((delegate, priority))) => CurrentHeadState::Prechecked { baker: delegate.clone(), priority },
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
    Ok(if timestamp < prev_timestamp - block_times.0 {
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
