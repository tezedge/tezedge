// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![no_std]

#[macro_use]
extern crate alloc;

#[cfg(test)]
extern crate std;

mod timestamp;
pub use self::timestamp::{Timestamp, Timing, TimingLinearGrow};

mod validator;
pub use self::validator::{Validator, Votes, ProposerMap};

mod timeout;
pub use self::timeout::{Config, TimeHeader};

mod block;
pub use self::block::{PreCertificate, Certificate, Block, Payload};

mod event;
pub use self::event::{BlockId, Event, Action, LogRecord, LogLevel};

mod machine;
pub use self::machine::Machine;
