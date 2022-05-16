// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

mod command_line;
pub use self::command_line::{Arguments, Command};

mod proof_of_work;

pub mod machine;

mod services;
pub use self::services::{EventWithTime, Services};

pub use tenderbake::Timestamp;
