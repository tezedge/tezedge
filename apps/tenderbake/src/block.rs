// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use alloc::vec::Vec;

use serde::{Serialize, Deserialize};

use crypto::hash::{BlockHash, BlockPayloadHash as PayloadHash};

use super::{timeout::TimeHeader, validator::Votes};

#[derive(Clone, Serialize, Deserialize)]
pub struct PreCertificate<Id, Op>
where
    Id: Ord,
{
    pub payload_hash: PayloadHash,
    pub payload_round: i32,
    pub votes: Votes<Id, Op>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Certificate<Id, Op>
where
    Id: Ord,
{
    pub votes: Votes<Id, Op>,
}

#[derive(Clone)]
pub struct Block<Id, Op>
where
    Id: Ord,
{
    pub pred_hash: BlockHash,
    pub hash: BlockHash,
    pub level: i32,
    pub time_header: TimeHeader<false>,
    pub payload: Option<Payload<Id, Op>>,
}

#[derive(Clone)]
pub struct Payload<Id, Op>
where
    Id: Ord,
{
    pub hash: PayloadHash,
    pub payload_round: i32,
    pub pre_cer: Option<PreCertificate<Id, Op>>,
    pub cer: Option<Certificate<Id, Op>>,
    pub operations: Vec<Op>,
}
