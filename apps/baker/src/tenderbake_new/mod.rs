// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

pub mod hash;
pub use self::hash::NoValue;

mod timestamp;
pub use self::timestamp::{Config, TimeHeader, Timestamp, Timing, TimingLinearGrow};

mod types;
pub use self::types::{Action, Block, BlockId, Event, LogLevel, Payload};
pub use self::types::{Certificate, PreCertificate, Vote, Votes};
pub use self::types::{ConsensusBody, OperationSimple, WithH};

mod machine;
pub use self::machine::Machine;

/// Map that provides a proposer for given level and round
pub trait ProposerMap {
    /// the closest round where we have proposer
    fn proposer(&self, level: i32, round: i32) -> Option<(i32, hash::ContractTz1Hash)>;
}

#[cfg(all(test, feature = "testing-mock"))]
mod tests;
