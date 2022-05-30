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
    use tenderbake::Timing;

    use super::{BakerAction, BakerState, OperationsEventAction};
    use crate::{services::event::OperationSimple, EventWithTime};

    fn test_initialized<F>(f: F)
    where
        F: FnOnce(BakerState, i32, Vec<(u16, usize)>),
    {
        let state =
            serde_json::from_str::<BakerState>(include_str!("test_data/state.json")).unwrap();
        let tb_state = &state.as_ref().tb_state;
        let tb_config = &state.as_ref().tb_config;

        let level = tb_state.level().unwrap();

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

        f(state, level, validators.collect())
    }

    mod quorum {
        use crate::machine::{ScheduleTimeoutAction, TickEventAction};

        use super::*;

        #[test]
        fn simplest() {
            test_initialized(|state, level, validators| {
                let tb_state = &state.as_ref().tb_state;
                let tb_config = &state.as_ref().tb_config;

                let quorum_size = tb_config.quorum as usize;
                let predecessor_hash = tb_state.predecessor_hash().unwrap();
                let payload_hash = tb_state.payload_hash().unwrap();
                let round = tb_state.round().unwrap();
                let preendorsements = {
                    BakerAction::OperationsEvent(OperationsEventAction {
                        operations: validators
                            .iter()
                            .map(|&(slot, _)| {
                                OperationSimple::preendorsement(
                                    &predecessor_hash,
                                    &payload_hash,
                                    level,
                                    round,
                                    slot,
                                )
                            })
                            .collect(),
                    })
                };

                let now = tb_state.timestamp().unwrap();
                let state = state.handle_event(EventWithTime {
                    action: preendorsements,
                    now,
                });

                let endorsement = |slot| {
                    BakerAction::OperationsEvent(OperationsEventAction {
                        operations: vec![OperationSimple::endorsement(
                            &predecessor_hash,
                            &payload_hash,
                            level,
                            round,
                            slot,
                        )],
                    })
                };

                let mut total_power = 0;
                let mut state = state;
                state.as_mut().actions.clear();
                for (slot, power) in validators {
                    state = state.handle_event(EventWithTime {
                        action: endorsement(slot),
                        now,
                    });
                    total_power += power;

                    if total_power >= quorum_size {
                        let deadline = state
                            .as_ref()
                            .actions
                            .iter()
                            .find_map(|a| match a {
                                BakerAction::ScheduleTimeout(ScheduleTimeoutAction {
                                    deadline,
                                }) => Some(*deadline),
                                _ => None,
                            })
                            .unwrap();

                        state = state.handle_event(EventWithTime {
                            action: BakerAction::TickEvent(TickEventAction {
                                scheduled_at_level: level,
                                scheduled_at_round: round,
                            }),
                            now: deadline,
                        });

                        let propose_action = state
                            .as_ref()
                            .actions
                            .iter()
                            .find(|a| matches!(a, &BakerAction::Propose(_)));
                        assert!(propose_action.is_some());

                        return;
                    } else {
                        assert!(state.as_ref().actions.is_empty());
                    }
                }
            })
        }

        #[test]
        fn late_endorsements() {
            test_initialized(|state, level, validators| {
                let tb_state = &state.as_ref().tb_state;
                let tb_config = &state.as_ref().tb_config;

                let predecessor_hash = tb_state.predecessor_hash().unwrap();
                let payload_hash = tb_state.payload_hash().unwrap();
                let round = tb_state.round().unwrap();
                let preendorsements = {
                    BakerAction::OperationsEvent(OperationsEventAction {
                        operations: validators
                            .iter()
                            .map(|&(slot, _)| {
                                OperationSimple::preendorsement(
                                    &predecessor_hash,
                                    &payload_hash,
                                    level,
                                    round,
                                    slot,
                                )
                            })
                            .collect(),
                    })
                };

                let endorsement = |slot| {
                    BakerAction::OperationsEvent(OperationsEventAction {
                        operations: vec![OperationSimple::endorsement(
                            &predecessor_hash,
                            &payload_hash,
                            level,
                            round,
                            slot,
                        )],
                    })
                };

                let round_duration = tb_config.timing.round_duration(round);
                let mut now = tb_state.timestamp().unwrap();
                let mut state = state.handle_event(EventWithTime {
                    action: preendorsements,
                    now,
                });

                let mut actions = vec![];
                for (n, (slot, _)) in validators.into_iter().enumerate() {
                    // add endorsements number 0, 1, 2, 3, it should be enough,
                    // but then add endorsement 4, and see it will also included
                    if n == 4 {
                        now += round_duration / 2;
                    }
                    state = state.handle_event(EventWithTime {
                        action: endorsement(slot),
                        now,
                    });
                    actions.append(&mut state.as_mut().actions);
                }

                let deadline = actions
                    .iter()
                    .find_map(|a| match a {
                        BakerAction::ScheduleTimeout(ScheduleTimeoutAction { deadline }) => {
                            Some(*deadline)
                        }
                        _ => None,
                    })
                    .unwrap();

                state = state.handle_event(EventWithTime {
                    action: BakerAction::TickEvent(TickEventAction {
                        scheduled_at_level: level,
                        scheduled_at_round: round,
                    }),
                    now: deadline,
                });

                let propose_action = state
                    .as_ref()
                    .actions
                    .iter()
                    .find_map(|a| match a {
                        BakerAction::Propose(propose_action) => Some(propose_action.clone()),
                        _ => None,
                    })
                    .unwrap();

                let consensus_operations = &propose_action.operations[0];
                assert_eq!(
                    consensus_operations.len(),
                    5,
                    "late endorsements are not included"
                );
            })
        }
    }

    mod prequorum {
        use super::*;

        #[test]
        fn simplest() {
            test_initialized(simplest_inner)
        }

        fn simplest_inner(state: BakerState, level: i32, validators: Vec<(u16, usize)>) {
            let st = state.as_ref();
            let quorum_size = st.tb_config.quorum as usize;
            let predecessor_hash = st.tb_state.predecessor_hash().unwrap();
            let payload_hash = st.tb_state.payload_hash().unwrap();
            let round = st.tb_state.round().unwrap();

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

            let now = st.tb_state.timestamp().unwrap();
            let mut total_power = 0;
            let mut state = state;
            for (slot, power) in validators {
                state = state.handle_event(EventWithTime {
                    action: preendorsement(slot),
                    now,
                });
                total_power += power;

                if total_power >= quorum_size {
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
            test_initialized(outdated_inner)
        }

        fn outdated_inner(state: BakerState, level: i32, validators: Vec<(u16, usize)>) {
            let st = state.as_ref();
            let quorum_size = st.tb_config.quorum as usize;
            let predecessor_hash = st.tb_state.predecessor_hash().unwrap();
            let payload_hash = st.tb_state.payload_hash().unwrap();
            let round = st.tb_state.round().unwrap();

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

            let round_duration = st.tb_config.timing.round_duration(round);
            let mut now = st.tb_state.timestamp().unwrap();
            let mut total_power = 0;
            let mut state = state;
            for (n, (slot, power)) in validators.into_iter().enumerate() {
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

                if total_power >= quorum_size {
                    // still we have no quorum, because got outdated preendorsement
                    assert!(state.as_ref().actions.is_empty());
                } else {
                    assert!(state.as_ref().actions.is_empty());
                }
            }
        }
    }
}
