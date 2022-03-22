// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::fmt;
use alloc::vec::Vec;

use super::{timeout::TimeHeader, validator::Votes};

#[derive(Clone, PartialEq, Eq)]
pub struct PayloadHash(pub [u8; 32]);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockHash(pub [u8; 32]);

#[derive(Clone)]
pub struct PreCertificate<Id, Op> {
    pub payload_hash: PayloadHash,
    pub payload_round: i32,
    pub votes: Votes<Id, Op>,
}

#[derive(Clone)]
pub struct Certificate<Id, Op> {
    pub votes: Votes<Id, Op>,
}

#[derive(Clone)]
pub struct Block<Id, Op> {
    pub pred_hash: BlockHash,
    pub hash: BlockHash,
    pub level: i32,
    pub time_header: TimeHeader<false>,
    pub payload: Option<Payload<Id, Op>>,
}

impl<Id, Op> fmt::Display for Block<Id, Op> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.level, self.time_header)
    }
}

#[derive(Clone)]
pub struct Payload<Id, Op> {
    pub hash: PayloadHash,
    pub payload_round: i32,
    pub pre_cer: Option<PreCertificate<Id, Op>>,
    pub cer: Option<Certificate<Id, Op>>,
    pub operations: Vec<Op>,
}
