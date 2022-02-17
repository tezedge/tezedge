// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::convert::TryInto;

use crypto::hash::BlockHash;
use tezos_messages::p2p::{binary_message::MessageHash, encoding::peer::PeerMessage};

use crate::{
    block_applier::BlockApplierApplyState,
    peer::message::read::PeerMessageReadSuccessAction,
    rights::{rights_actions::RightsGetAction, RightsKey},
    stats::current_head::stats_current_head_actions::StatsCurrentHeadReceivedAction,
    Action,
};

use super::{current_head_actions::*, BakingPriorityError, CurrentHeadState};

pub fn current_head_effects<S>(store: &mut crate::Store<S>, action: &crate::ActionWithMeta)
where
    S: crate::Service,
{
    match &action.action {
        Action::PeerMessageReadSuccess(PeerMessageReadSuccessAction { address, message }) => {
            let current_head = if let PeerMessage::CurrentHead(current_head) = message.message() {
                current_head
            } else {
                return;
            };

            let current_block_header = current_head.current_block_header();
            let block_hash = match current_block_header.message_typed_hash::<BlockHash>() {
                Ok(v) => v,
                Err(_) => return,
            };

            store.dispatch(StatsCurrentHeadReceivedAction {
                address: *address,
                level: current_block_header.level(),
                hash: block_hash.clone(),
                block_timestamp: current_block_header.timestamp().try_into().unwrap_or(0)
                    * 1_000_000_000,
                receive_timestamp: action.id,
                empty_mempool: current_head.current_mempool().is_empty(),
            });

            store.dispatch(CurrentHeadReceivedAction {
                address: *address,
                block_hash,
                block_header: current_block_header.clone(),
            });
        }
        Action::BlockApplierApplySuccess(_) => {
            let block = match &store.state().block_applier.current {
                BlockApplierApplyState::Success { block, .. } => block,
                _ => return,
            };

            let block_hash = block.hash.clone();
            let level = block.header.level();
            let timestamp = block.header.timestamp();
            store.dispatch(CurrentHeadApplyAction {
                block_hash,
                level,
                timestamp,
            });
        }

        Action::CurrentHeadReceived(CurrentHeadReceivedAction { block_hash, .. }) => {
            if let Some(CurrentHeadState::Received { .. }) =
                store.state.get().current_heads.candidates.get(block_hash)
            {
                store.dispatch(CurrentHeadPrecheckAction {
                    block_hash: block_hash.clone(),
                });
            }
        }
        Action::CurrentHeadPrecheck(CurrentHeadPrecheckAction { block_hash }) => {
            match store.state.get().current_heads.candidates.get(block_hash) {
                Some(CurrentHeadState::Prechecked {
                    block_header: _,
                    baker,
                    priority,
                }) => {
                    let baker = baker.clone();
                    let priority = *priority;
                    store.dispatch(CurrentHeadPrecheckSuccessAction {
                        block_hash: block_hash.clone(),
                        baker,
                        priority,
                    });
                }
                Some(CurrentHeadState::Rejected) => {
                    store.dispatch(CurrentHeadPrecheckRejectedAction {
                        block_hash: block_hash.clone(),
                    });
                }
                Some(CurrentHeadState::Error { error }) => {
                    let error = error.clone();
                    store.dispatch(CurrentHeadErrorAction {
                        block_hash: block_hash.clone(),
                        error,
                    });
                }
                _ => (),
            };
        }
        Action::CurrentHeadError(CurrentHeadErrorAction { block_hash, error }) => {
            slog::error!(&store.state().log, "current head error"; "block_hash" => block_hash.to_base58_check(), "error" => error.to_string());
        }

        Action::CurrentHeadApply(CurrentHeadApplyAction { .. }) => {
            store.dispatch(CurrentHeadPrecacheBakingRightsAction {});
        }
        Action::CurrentHeadPrecacheBakingRights(CurrentHeadPrecacheBakingRightsAction {
            ..
        }) => {
            if let Some((current_block_hash, level, prev_timestamp)) = store
                .state
                .get()
                .current_heads
                .applied_head()
                .map(|applied_head| {
                    (
                        applied_head.block_hash.clone(),
                        applied_head.level,
                        applied_head.timestamp,
                    )
                })
            {
                let max_priority = match max_priority_to_precache(
                    prev_timestamp,
                    (20, 30),
                    action.duration_since_epoch().as_secs(),
                ) {
                    Ok(v) => v,
                    Err(err) => {
                        slog::error!(&store.state.get().log, "error calculating max priority"; "error" => err.to_string());
                        return;
                    }
                };
                store.dispatch(RightsGetAction {
                    key: RightsKey::baking(current_block_hash, Some(level), Some(max_priority)),
                });
            }
        }
        _ => (),
    }
}

fn max_priority_to_precache(
    prev_timestamp: i64,
    block_times: (i64, i64),
    now: u64,
) -> Result<u16, BakingPriorityError> {
    let now: i64 = now.try_into()?;
    let priority: u16 = if now < prev_timestamp + block_times.0 {
        0
    } else {
        ((now - prev_timestamp - block_times.0) / block_times.1 + 1).try_into()?
    };
    Ok(priority)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::current_head::current_head_reducer::TIME_BETWEEN_BLOCKS;

    #[test]
    fn test_max_priority_to_precache_0() {
        let prev_timestamp = 100;
        let now = prev_timestamp + 1;
        let res = max_priority_to_precache(prev_timestamp, TIME_BETWEEN_BLOCKS, now as u64);
        assert_eq!(res, Ok(0));

        let now = prev_timestamp + TIME_BETWEEN_BLOCKS.0 - 1;
        let res = max_priority_to_precache(prev_timestamp, TIME_BETWEEN_BLOCKS, now as u64);
        assert_eq!(res, Ok(0));
    }

    #[test]
    fn test_max_priority_to_precache_1() {
        let prev_timestamp = 100;
        let now = prev_timestamp + TIME_BETWEEN_BLOCKS.0;
        let res = max_priority_to_precache(prev_timestamp, TIME_BETWEEN_BLOCKS, now as u64);
        assert_eq!(res, Ok(1));
    }

    #[test]
    fn test_max_priority_to_precache_n() {
        const N: u16 = 10;

        let prev_timestamp = 100;
        let now = prev_timestamp + TIME_BETWEEN_BLOCKS.0 + TIME_BETWEEN_BLOCKS.1 * (N as i64) - 1;
        let res = max_priority_to_precache(prev_timestamp, TIME_BETWEEN_BLOCKS, now as u64);
        assert_eq!(res, Ok(N));

        let prev_timestamp = 100;
        let now = prev_timestamp + TIME_BETWEEN_BLOCKS.0 + TIME_BETWEEN_BLOCKS.1 * (N as i64);
        let res = max_priority_to_precache(prev_timestamp, TIME_BETWEEN_BLOCKS, now as u64);
        assert_eq!(res, Ok(N + 1));
    }
}
