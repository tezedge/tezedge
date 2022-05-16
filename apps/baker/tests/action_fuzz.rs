// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![cfg(feature = "fuzzing")]
#![feature(backtrace)]
#![feature(alloc_error_hook)]
#![cfg_attr(test, feature(no_coverage))]

use std::sync::RwLock;

use fuzzcheck::{DefaultMutator, SerdeSerializer};
use once_cell::sync::Lazy;

use baker::{machine::*, Timestamp};
use serde::{Serialize, Deserialize};

pub static FUZZER_ARGS: Lazy<RwLock<Option<fuzzcheck::Arguments>>> =
    Lazy::new(|| RwLock::new(None));

pub static FUZZER_STATE: Lazy<RwLock<(BakerStateEjectable, Timestamp)>> = Lazy::new(|| {
    let state = serde_json::from_str::<BakerState>(include_str!("state.json")).unwrap();
    let timestamp = state.as_ref().tb_state.timestamp().unwrap();
    RwLock::new((BakerStateEjectable(Some(state)), timestamp))
});

#[derive(Clone, Debug, Serialize, Deserialize, fuzzcheck::DefaultMutator)]
pub enum AllActionsTest {
    IdleEvent(IdleEventAction),
    ProposalEvent(ProposalEventAction),
    SlotsEvent(SlotsEventAction),
    OperationsForBlockEvent(OperationsForBlockEventAction),
    LiveBlocksEvent(LiveBlocksEventAction),
    OperationsEvent(OperationsEventAction),
    TickEvent(TickEventAction),
}

#[cfg(test)]
#[test]
fn test_baker() {
    use std::time::Duration;

    use baker::EventWithTime;

    fn action_test_all(action_test: &AllActionsTest) {
        let timestamp = FUZZER_STATE
            .read()
            .unwrap()
            .1;
        let timestamp = timestamp + Duration::from_millis(1);

        let event = EventWithTime {
            action: match action_test {
                AllActionsTest::IdleEvent(act) => BakerAction::IdleEvent(act.clone()),
                AllActionsTest::ProposalEvent(act) => BakerAction::ProposalEvent(act.clone()),
                AllActionsTest::SlotsEvent(act) => BakerAction::SlotsEvent(act.clone()),
                AllActionsTest::OperationsForBlockEvent(act) => BakerAction::OperationsForBlockEvent(act.clone()),
                AllActionsTest::LiveBlocksEvent(act) => BakerAction::LiveBlocksEvent(act.clone()),
                AllActionsTest::OperationsEvent(act) => BakerAction::OperationsEvent(act.clone()),
                AllActionsTest::TickEvent(act) => BakerAction::TickEvent(act.clone()),
            },
            now: timestamp
        };

        let mut state_container = FUZZER_STATE.write().unwrap();
        let state = state_container.0.0.take().unwrap();
        let state = state.handle_event(event);
        *state_container = (BakerStateEjectable(Some(state)), timestamp);
    }

    pub fn handle_alloc_error(layout: std::alloc::Layout) {
        let bt = std::backtrace::Backtrace::force_capture();
        println!("Allocation error {:?}", layout.size());
        println!("{:?}", bt);
        //std::process::exit(1)
    }

    std::alloc::set_alloc_error_hook(handle_alloc_error);

    let builder = fuzzcheck::fuzz_test(action_test_all)
        .mutator(AllActionsTest::default_mutator())
        .serializer(SerdeSerializer::default())
        .default_sensor_and_pool()
        .arguments_from_cargo_fuzzcheck();

    // *FUZZER_ARGS.write().unwrap() = Some(builder.arguments.clone());
    builder.stop_after_first_test_failure(true).launch();
}
