// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![forbid(unsafe_code)]
#![cfg_attr(feature = "fuzzing", feature(no_coverage))]

mod proof_of_work;

pub mod machine;

mod services;
pub use self::services::{
    client::{Protocol, ProtocolBlockHeaderI, ProtocolBlockHeaderJ},
    EventWithTime, Services,
};

pub use tenderbake::Timestamp;
