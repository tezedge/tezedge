// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![no_std]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

#[macro_use]
extern crate alloc;

#[cfg(any(test, feature = "fuzzing"))]
extern crate std;

mod timestamp;
pub use self::timestamp::{Timestamp, Timing, TimingLinearGrow};

mod validator;
pub use self::validator::{Validator, Votes, ProposerMap};

mod timeout;
pub use self::timeout::{Config, TimeHeader, Timeout};

mod block;
pub use self::block::{PreCertificate, Certificate, Block, Payload};

mod event;
pub use self::event::{BlockId, Event, Action, LogRecord, LogLevel};

mod machine;
pub use self::machine::Machine;
