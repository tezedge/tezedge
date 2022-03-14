// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{time::Duration, fmt};
use alloc::boxed::Box;

use super::{
    validator::Validator,
    timestamp::Timestamp,
    block::{BlockId, Payload, BlockInfo},
};

/// Configuration of the algorithm
pub struct Config {
    /// threshold, how many votes required to consider the quorum is reached
    pub consensus_threshold: u32,
    /// duration of round 0
    pub minimal_block_delay: Duration,
    /// duration increase per round
    pub delay_increment_per_round: Duration,
}

impl Config {
    /// duration of the given round
    pub fn round_duration(&self, round: i32) -> Duration {
        self.minimal_block_delay + self.delay_increment_per_round * (round as u32)
    }

    /// delay to give a chance to accept block for a new level, before try to start a new round
    pub fn delay(&self) -> Duration {
        (self.minimal_block_delay / 5).min(Duration::from_secs(1))
    }

    pub fn round(&self, now: Timestamp, start_this_level: Timestamp) -> i32 {
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
        let d = self.delay_increment_per_round.as_secs_f64();
        let m = self.minimal_block_delay.as_secs_f64();
        let p = d - 2.0 * m;
        let r = (p + libm::sqrt(p * p + 8.0 * d * e)) / (2.0 * d);
        libm::floor(r) as i32
    }
}

/// Proposal of new head and its predecessor
pub struct Proposal<P>
where
    P: Payload,
{
    pub pred_timestamp: Timestamp,
    pub pred_round: i32,
    pub head: BlockInfo<P>,
}

pub struct Preendorsement {
    pub validator: Validator,
    pub block_id: BlockId,
}

impl fmt::Display for Preendorsement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.block_id, self.validator)
    }
}

pub struct Endorsement {
    pub validator: Validator,
    pub block_id: BlockId,
}

impl fmt::Display for Endorsement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.block_id, self.validator)
    }
}

pub enum Event<P>
where
    P: Payload,
{
    Proposal(Box<Proposal<P>>, Timestamp),
    Preendorsement(Preendorsement, P::Item),
    Endorsement(Endorsement, P::Item, Timestamp),
    Timeout,
    PayloadItem(P::Item),
}

pub enum Action<P>
where
    P: Payload,
{
    ScheduleTimeout(Timestamp),
    Preendorse {
        pred_hash: [u8; 32],
        content: Preendorsement,
    },
    Endorse {
        pred_hash: [u8; 32],
        content: Endorsement,
    },
    Propose(Box<BlockInfo<P>>),
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::{Config, Timestamp};

    #[test]
    fn time() {
        let config = Config {
            consensus_threshold: 0,
            minimal_block_delay: Duration::from_secs(5),
            delay_increment_per_round: Duration::from_secs(1),
        };
        let now = Timestamp {
            unix_epoch: Duration::from_secs(5),
        };
        let start_of_this_level = Timestamp {
            unix_epoch: Duration::ZERO,
        };
        let r = config.round(now, start_of_this_level);
        assert_eq!(r, 2);
    }
}
