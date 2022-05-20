// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

mod actions;
mod effects;
mod reducer;

mod cycle_nonce;
mod request;
mod state;

pub use self::{
    actions::*,
    effects::baker_effects,
    reducer::baker_reducer,
    state::{BakerState, BakerStateEjectable},
};

#[cfg(test)]
mod tests {
    use super::{BakerAction, BakerState, OperationsEventAction};
    use crate::{services::event::OperationSimple, EventWithTime};

    use tenderbake::Timing;

    mod prequorum {
        use super::*;

        #[test]
        fn simplest() {
            let state = serde_json::from_str::<BakerState>(include_str!("state.json")).unwrap();
            let tb_state = &state.as_ref().tb_state;
            let tb_config = &state.as_ref().tb_config;

            let quorum = tb_config.quorum as usize;
            let level = tb_state.level().unwrap();
            let round = tb_state.round().unwrap();
            let timestamp = tb_state.timestamp().unwrap();
            let predecessor_hash = tb_state.predecessor_hash().unwrap();
            let payload_hash = tb_state.payload_hash().unwrap();

            // take delegates for the level
            let validators = {
                tb_config
                    .map
                    .delegates
                    .get(&level)
                    .unwrap()
                    .clone()
                    .into_values()
                    .map(|s| (s.0[0], s.0.len()))
            };

            let preendorsement = |slot| {
                BakerAction::OperationsEvent(OperationsEventAction {
                    operations: vec![OperationSimple::preendorsement(
                        &predecessor_hash,
                        &payload_hash,
                        level,
                        round,
                        slot,
                    )],
                })
            };

            let now = timestamp;
            let mut total_power = 0;
            let mut state = state;
            for (slot, power) in validators {
                state = state.handle_event(EventWithTime {
                    action: preendorsement(slot),
                    now,
                });
                total_power += power;

                if total_power >= quorum {
                    let endorse = state
                        .as_ref()
                        .actions
                        .iter()
                        .find(|a| matches!(a, BakerAction::Vote(_)));
                    assert!(endorse.is_some());
                    return;
                } else {
                    assert!(state.as_ref().actions.is_empty());
                }
            }
        }

        #[test]
        fn outdated() {
            let state = serde_json::from_str::<BakerState>(include_str!("state.json")).unwrap();
            let tb_state = &state.as_ref().tb_state;
            let tb_config = &state.as_ref().tb_config;

            let quorum = tb_config.quorum as usize;
            let level = tb_state.level().unwrap();
            let round = tb_state.round().unwrap();
            let timestamp = tb_state.timestamp().unwrap();
            let predecessor_hash = tb_state.predecessor_hash().unwrap();
            let payload_hash = tb_state.payload_hash().unwrap();
            let round_duration = tb_config.timing.round_duration(round);

            // take delegates for the level
            let validators = {
                tb_config
                    .map
                    .delegates
                    .get(&level)
                    .unwrap()
                    .clone()
                    .into_values()
                    .map(|s| (s.0[0], s.0.len()))
            };

            let preendorsement = |slot| {
                BakerAction::OperationsEvent(OperationsEventAction {
                    operations: vec![OperationSimple::preendorsement(
                        &predecessor_hash,
                        &payload_hash,
                        level,
                        round,
                        slot,
                    )],
                })
            };

            let mut now = timestamp;
            let mut total_power = 0;
            let mut state = state;
            for (n, (slot, power)) in validators.enumerate() {
                if n == 3 {
                    // go to next round, further preendorsements are outdated
                    // preendorsements 0, 1, 2 accounted, but 3 and 4 was not
                    now += round_duration;
                }
                state = state.handle_event(EventWithTime {
                    action: preendorsement(slot),
                    now,
                });
                total_power += power;

                if total_power >= quorum {
                    // still we have no quorum, because got outdated preendorsement
                    assert!(state.as_ref().actions.is_empty());
                } else {
                    assert!(state.as_ref().actions.is_empty());
                }
            }
        }
    }
}
