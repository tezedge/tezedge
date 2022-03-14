// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![no_std]

extern crate alloc;

mod timestamp;
pub use self::timestamp::Timestamp;

mod validator;
pub use self::validator::{Validator, ValidatorMap};

mod block;
pub use self::block::{BlockId, Votes, Prequorum, Quorum, Payload, BlockInfo};

mod interface;
pub use self::interface::{Config, Action, Event, Proposal, Preendorsement, Endorsement};

mod state;
pub use self::state::Machine;
