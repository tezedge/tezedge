// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use crate::baker::{CONSENSUS_COMMITTEE_SIZE, DELAY_INCREMENT_PER_ROUND, MINIMAL_BLOCK_DELAY};
use crate::{Action, ActionWithMeta, State};

use super::{BakerBlockBakerState, BakingSlot};

pub fn baker_block_baker_reducer(state: &mut State, action: &ActionWithMeta) {
    match &action.action {
        Action::CurrentHeadRehydrated(_) | Action::CurrentHeadUpdate(_) => {
            for (_, baker) in state.bakers.iter_mut() {
                baker.block_baker = BakerBlockBakerState::Idle {
                    time: action.time_as_nanos(),
                };
                baker.elected_block = None;
            }
        }
        Action::BakerBlockBakerRightsGetPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                baker.block_baker = BakerBlockBakerState::RightsGetPending {
                    time: action.time_as_nanos(),
                    slots: None,
                    next_slots: None,
                };
            }
        }
        Action::BakerBlockBakerRightsGetCurrentLevelSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::RightsGetPending { slots, .. } => {
                        *slots = Some(content.slots.clone());
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerRightsGetNextLevelSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::RightsGetPending { next_slots, .. } => {
                        *next_slots = Some(content.slots.clone());
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerRightsGetSuccess(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &mut baker.block_baker {
                    BakerBlockBakerState::RightsGetPending {
                        slots, next_slots, ..
                    } => match (slots, next_slots) {
                        (Some(slots), Some(next_slots)) => {
                            baker.block_baker = BakerBlockBakerState::RightsGetSuccess {
                                time: action.time_as_nanos(),
                                slots: std::mem::take(slots),
                                next_slots: std::mem::take(next_slots),
                            };
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerRightsNoRights(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let time = action.time_as_nanos();
                baker.block_baker = BakerBlockBakerState::NoRights { time };
            }
        }
        Action::BakerBlockBakerTimeoutPending(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                let round = match state.current_head.round() {
                    Some(v) => v,
                    None => return,
                };
                let timestamp = match state.current_head.get() {
                    Some(v) => v.header.timestamp().as_u64() * 1_000_000_000,
                    None => return,
                };
                match &baker.block_baker {
                    BakerBlockBakerState::RightsGetSuccess {
                        slots, next_slots, ..
                    } => {
                        let current_slot = (round as u32 % CONSENSUS_COMMITTEE_SIZE) as u16;
                        let next_round = slots
                            .into_iter()
                            .map(|slot| *slot)
                            .find(|slot| *slot > current_slot)
                            .and_then(|slot| {
                                let rounds_left = slot.checked_sub(current_slot)? as i32;
                                let target_round = round + rounds_left;
                                let time_left =
                                    calc_time_until_round(round as u64, target_round as u64);
                                let timeout = timestamp + time_left;
                                Some(BakingSlot { slot, timeout })
                            });
                        let next_level = next_slots.get(0).cloned().map(|slot| {
                            let time_left = calc_time_until_round(round as u64, slot as u64);
                            let timeout = timestamp + time_left;
                            BakingSlot { slot, timeout }
                        });

                        baker.block_baker = BakerBlockBakerState::TimeoutPending {
                            time: action.time_as_nanos(),
                            next_round,
                            next_level,
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerBakeNextLevel(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::TimeoutPending { next_level, .. } => {
                        let next_level = match next_level {
                            Some(v) => v,
                            None => return,
                        };
                        baker.block_baker = BakerBlockBakerState::BakeNextLevel {
                            time: action.time_as_nanos(),
                            slot: next_level.slot,
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::BakerBlockBakerBakeNextRound(content) => {
            if let Some(baker) = state.bakers.get_mut(&content.baker) {
                match &baker.block_baker {
                    BakerBlockBakerState::TimeoutPending { next_round, .. } => {
                        let next_round = match next_round {
                            Some(v) => v,
                            None => return,
                        };
                        baker.block_baker = BakerBlockBakerState::BakeNextRound {
                            time: action.time_as_nanos(),
                            slot: next_round.slot,
                        };
                    }
                    _ => {}
                }
            }
        }
        Action::MempoolQuorumReached(_) => {
            let head = state.current_head.get().cloned();
            state
                .bakers
                .iter_mut()
                .filter(|(_, baker_state)| baker_state.elected_block.is_none())
                .for_each(|(_, baker_state)| baker_state.elected_block = head.clone());
        }
        _ => {}
    }
}

fn calc_seconds_until_round(current_round: u64, target_round: u64) -> u64 {
    // let current_slot = (current_round as u32 % CONSENSUS_COMMITTEE_SIZE) as u16;
    let rounds_left = target_round.saturating_sub(current_round);
    MINIMAL_BLOCK_DELAY * rounds_left
        + DELAY_INCREMENT_PER_ROUND * rounds_left * (current_round + target_round).saturating_sub(1)
            / 2
}

fn calc_time_until_round(current_round: u64, target_round: u64) -> u64 {
    calc_seconds_until_round(current_round, target_round) * 1_000_000_000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calc_seconds_until_round() {
        assert_eq!(calc_seconds_until_round(0, 0), 0);
        assert_eq!(calc_seconds_until_round(0, 1), 15);
        assert_eq!(calc_seconds_until_round(0, 2), 35);
        assert_eq!(calc_seconds_until_round(0, 3), 60);
        assert_eq!(calc_seconds_until_round(0, 4), 90);
        assert_eq!(calc_seconds_until_round(0, 5), 125);

        assert_eq!(calc_seconds_until_round(1, 2), 20);
        assert_eq!(calc_seconds_until_round(1, 3), 45);
        assert_eq!(calc_seconds_until_round(1, 4), 75);
        assert_eq!(calc_seconds_until_round(1, 5), 110);

        assert_eq!(calc_seconds_until_round(2, 3), 25);
        assert_eq!(calc_seconds_until_round(2, 4), 55);
        assert_eq!(calc_seconds_until_round(2, 5), 90);

        assert_eq!(calc_seconds_until_round(3, 4), 30);
        assert_eq!(calc_seconds_until_round(3, 5), 65);

        assert_eq!(calc_seconds_until_round(4, 5), 35);
    }
}
