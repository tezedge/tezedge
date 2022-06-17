// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    fmt,
    ops::{Add, AddAssign, Sub},
    time::Duration,
};

use serde::{Deserialize, Serialize};

use super::{hash::ContractTz1Hash, types::Action, ProposerMap};

/// Timestamp as a unix time
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[cfg(not(feature = "testing-mock"))]
pub struct Timestamp {
    unix_epoch: Duration,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[cfg(feature = "testing-mock")]
pub struct Timestamp {
    minutes: u8,
    seconds: u8,
}

#[cfg(not(feature = "testing-mock"))]
impl Timestamp {
    pub fn from_unix_epoch(unix_epoch: Duration) -> Self {
        Timestamp { unix_epoch }
    }

    pub fn unix_epoch(self) -> Duration {
        self.unix_epoch
    }
}

#[cfg(feature = "testing-mock")]
impl Timestamp {
    pub fn from_unix_epoch(unix_epoch: Duration) -> Self {
        let s = unix_epoch.as_secs() % 3600;
        Timestamp {
            minutes: (s / 60) as u8,
            seconds: (s % 60) as u8,
        }
    }

    pub fn unix_epoch(self) -> Duration {
        Duration::from_secs(u64::from(self.minutes) * 60 + u64::from(self.seconds))
    }
}

#[derive(Serialize, Deserialize)]
struct TimestampHelper {
    hours: u64,
    minutes: u8,
    seconds: u8,
    nanos: u32,
}

impl From<TimestampHelper> for Timestamp {
    fn from(v: TimestampHelper) -> Self {
        let s = v.hours * 3600 + u64::from(v.minutes) * 60 + u64::from(v.seconds);
        Self::from_unix_epoch(Duration::new(s, v.nanos))
    }
}

impl From<Timestamp> for TimestampHelper {
    fn from(v: Timestamp) -> Self {
        TimestampHelper {
            hours: v.unix_epoch().as_secs() / 3600,
            minutes: ((v.unix_epoch().as_secs() % 3600) / 60) as u8,
            seconds: (v.unix_epoch().as_secs() % 60) as u8,
            nanos: v.unix_epoch().subsec_nanos(),
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let TimestampHelper {
            minutes, seconds, ..
        } = (*self).into();
        write!(f, "{:02}:{:02}", minutes, seconds)
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        TimestampHelper::from(*self).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        TimestampHelper::deserialize(deserializer).map(From::from)
    }
}
impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: Duration) -> Self::Output {
        Self::from_unix_epoch(self.unix_epoch() + rhs)
    }
}

impl AddAssign<Duration> for Timestamp {
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

impl Sub<Timestamp> for Timestamp {
    type Output = Duration;

    fn sub(self, rhs: Timestamp) -> Self::Output {
        self.unix_epoch() - rhs.unix_epoch()
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, rhs: Duration) -> Self::Output {
        Self::from_unix_epoch(self.unix_epoch() - rhs)
    }
}
pub trait Timing {
    // what is the duration of the round
    fn round_duration(&self, round: i32) -> Duration;

    // what is the round at time `now`
    fn round(&self, now: Timestamp, start_level: Timestamp) -> i32 {
        let mut round = 0;
        let mut t = start_level;
        loop {
            t += self.round_duration(round);
            if t > now {
                break round;
            }
            round += 1;
        }
    }

    // time offset of the round
    fn offset(&self, round: i32) -> Duration {
        (0..round).fold(Duration::ZERO, |t, round| t + self.round_duration(round))
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TimingLinearGrow {
    pub minimal_block_delay: Duration,
    pub delay_increment_per_round: Duration,
}

impl Timing for TimingLinearGrow {
    /// duration of the given round
    fn round_duration(&self, round: i32) -> Duration {
        self.minimal_block_delay + self.delay_increment_per_round * (round as u32)
    }

    fn round(&self, now: Timestamp, start_this_level: Timestamp) -> i32 {
        let elapsed = if now < start_this_level {
            Duration::ZERO
        } else {
            now - start_this_level
        };

        // m := minimal_block_delay
        // d := delay_increment_per_round
        // r := round
        // e := elapsed
        // duration(r) = m + d * r
        // e = duration(0) + duration(1) + ... + duration(r - 1)
        // e = m + (m + d) + (m + d * 2) + ... + (m + d * (r - 1))
        // e = m * r + d * r * (r - 1) / 2
        // d * r^2 + (2 * m - d) * r - 2 * e = 0

        let e = elapsed.as_secs_f64();
        let m = self.minimal_block_delay.as_secs_f64();
        let r = if self.delay_increment_per_round.is_zero() {
            e / m
        } else {
            let d = self.delay_increment_per_round.as_secs_f64();
            let p = d - 2.0 * m;
            (p + (p * p + 8.0 * d * e).sqrt()) / (2.0 * d)
        };

        r.floor() as i32
    }

    // in linear case we can optimize
    fn offset(&self, round: i32) -> Duration {
        self.minimal_block_delay * (round as u32)
            + self.delay_increment_per_round * (round * (round - 1) / 2) as u32
    }
}

/////////////////

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
        log: &mut Vec<Action>,
        config: &Config<T, P>,
        now: Timestamp,
        level: i32,
    ) -> Option<Timeout>
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
                    log.push(Action::WillBakeThisLevel { round, timestamp });
                } else {
                    log.push(Action::WillBakeNextLevel { round, timestamp });
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
pub struct Timeout {
    pub proposer: ContractTz1Hash,
    pub round: i32,
    pub timestamp: Timestamp,
}
