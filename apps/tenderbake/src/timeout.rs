// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::fmt;

use alloc::vec::Vec;
use serde::{Serialize, Deserialize};

use super::{
    timestamp::{Timestamp, Timing},
    validator::ProposerMap,
    event::LogRecord,
};

#[derive(Clone, Serialize, Deserialize)]
pub struct TimeHeader<const PREV: bool> {
    pub round: i32,
    pub timestamp: Timestamp,
}

impl<const THIS: bool> fmt::Display for TimeHeader<THIS> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = if THIS { "this level" } else { "next level" };
        write!(f, "{}, round: {}, {}", m, self.round, self.timestamp)
    }
}

impl<const PREV: bool> TimeHeader<PREV> {
    pub fn start_level<T>(&self, timing: &T) -> Timestamp
    where
        T: Timing,
    {
        self.timestamp + timing.round_duration(self.round)
    }

    pub fn round_local_coord<T>(&self, timing: &T, now: Timestamp) -> i32
    where
        T: Timing,
    {
        timing.round(now, self.start_level(timing))
    }

    pub fn timestamp_at_round<T>(&self, timing: &T, round: i32) -> Timestamp
    where
        T: Timing,
    {
        self.start_level(timing) + timing.offset(round)
    }

    pub fn calculate<T, P>(
        &self,
        log: &mut Vec<LogRecord>,
        config: &Config<T, P>,
        now: Timestamp,
        level: i32,
    ) -> Option<Timeout<P::Id>>
    where
        T: Timing,
        P: ProposerMap,
    {
        let (r, l) = if PREV { (1, 0) } else { (0, 1) };
        let current_round = self.round_local_coord(&config.timing, now);
        config
            .map
            .proposer(level + l, current_round + r)
            .map(|(round, proposer)| {
                let timestamp = self.timestamp_at_round(&config.timing, round);
                if PREV {
                    log.push(LogRecord::WillBakeThisLevel { round, timestamp });
                } else {
                    log.push(LogRecord::WillBakeNextLevel { round, timestamp });
                }
                Timeout {
                    proposer,
                    round,
                    timestamp,
                }
            })
    }
}

impl TimeHeader<false> {
    pub fn into_prev(self) -> TimeHeader<true> {
        let TimeHeader { round, timestamp } = self;
        TimeHeader { round, timestamp }
    }
}

impl TimeHeader<true> {
    pub fn into_this(self) -> TimeHeader<false> {
        let TimeHeader { round, timestamp } = self;
        TimeHeader { round, timestamp }
    }
}

#[derive(Serialize, Deserialize)]
pub struct Config<T, P> {
    pub timing: T,
    pub map: P,
    pub quorum: u32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Timeout<Id> {
    pub proposer: Id,
    pub round: i32,
    pub timestamp: Timestamp,
}
