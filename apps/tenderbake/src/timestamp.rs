// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{
    fmt,
    time::Duration,
    ops::{Add, Sub, AddAssign},
};

use serde::{Serialize, Deserialize};

/// Timestamp as a unix time
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
pub struct Timestamp {
    pub unix_epoch: Duration,
}

impl Timestamp {
    #[cfg(test)]
    pub fn new(minute: u64, second: u64) -> Self {
        Timestamp {
            unix_epoch: Duration::from_secs(minute * 60 + second),
        }
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let int = self.unix_epoch.as_secs() % 3600;
        let fr = self.unix_epoch.subsec_millis();
        let minutes = int / 60;
        let seconds = int % 60;
        write!(f, "{:02}:{:02}.{:03}", minutes, seconds, fr)
    }
}

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: Duration) -> Self::Output {
        Timestamp {
            unix_epoch: self.unix_epoch + rhs,
        }
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
        self.unix_epoch - rhs.unix_epoch
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, rhs: Duration) -> Self::Output {
        Timestamp {
            unix_epoch: self.unix_epoch - rhs,
        }
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
            (p + libm::sqrt(p * p + 8.0 * d * e)) / (2.0 * d)
        };

        libm::floor(r) as i32
    }

    // in linear case we can optimize
    fn offset(&self, round: i32) -> Duration {
        self.minimal_block_delay * (round as u32)
            + self.delay_increment_per_round * (round * (round - 1) / 2) as u32
    }
}

// #[cfg(test)]
// mod tests {
//     use core::time::Duration;

//     use super::{Timing, TimingLinearGrow};

//     #[test]
//     #[kani::proof]
//     fn positive_discriminant() {
//         let t = TimingLinearGrow {
//             minimal_block_delay: Duration::from_secs(1),
//             delay_increment_per_round: Duration::from_secs(1),
//         };

//         let now = Duration::from_secs(kani::any());
//         let start_level = Duration::from_secs(0);

//         t.round(now, start_level);
//     }
// }
