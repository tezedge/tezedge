// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![no_std]

#[macro_use]
extern crate alloc;

mod timestamp;
pub use self::timestamp::{Timestamp, Timing, TimingLinearGrow};

mod validator;
pub use self::validator::{Validator, Votes, ProposerMap};

mod block;
pub use self::block::{PayloadHash, BlockHash, PreCertificate, Certificate, Block, Payload, TimeHeader};

mod event;
pub use self::event::{BlockId, Event, Action, LogRecord, LogLevel};

mod machine;
pub use self::machine::Machine;
