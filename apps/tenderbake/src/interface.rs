// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt, time::Duration};

use super::{
    block::{BlockId, BlockInfo, Payload},
    timestamp::Timestamp,
    validator::Validator,
};

pub struct Config {
    pub consensus_threshold: u32,
    pub minimal_block_delay: Duration,
    pub delay_increment_per_round: Duration,
}

impl Config {
    pub fn round_duration(&self, round: i32) -> Duration {
        self.minimal_block_delay + self.delay_increment_per_round * (round as u32)
    }
}

#[derive(Debug)]
pub struct Proposal<P>
where
    P: Payload,
{
    pub pred: BlockInfo<P>,
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
    Timeout(Timestamp),
    TimeoutDelayed(Timestamp),
    Proposal(Box<Proposal<P>>, Timestamp),
    Preendorsement(Preendorsement, P::Item),
    Endorsement(Endorsement, P::Item),
    PayloadItem(P::Item),
}

pub enum Action<P>
where
    P: Payload,
{
    ScheduleTimeout(Timestamp),
    ScheduleTimeoutDelayed(Duration),
    Propose(Box<Proposal<P>>),
    Preendorse {
        pred_hash: [u8; 32],
        content: Preendorsement,
    },
    Endorse {
        pred_hash: [u8; 32],
        content: Endorsement,
    },
}
