// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::fmt;
use alloc::boxed::Box;

use serde::{Serialize, Deserialize};

use crypto::hash::{BlockHash, BlockPayloadHash as PayloadHash};

use super::{
    timestamp::Timestamp,
    validator::Validator,
    block::Block,
};

#[derive(PartialEq, Eq, Clone)]
pub struct BlockId {
    pub level: i32,
    pub round: i32,
    pub payload_hash: PayloadHash,
}

pub enum Event<Id, Op>
where
    Id: Ord,
{
    Proposal(Box<Block<Id, Op>>, Timestamp),
    PreVoted(BlockId, Validator<Id, Op>, Timestamp),
    Voted(BlockId, Validator<Id, Op>, Timestamp),
    Operation(Op),
    Timeout,
}

pub enum Action<Id, Op>
where
    Id: Ord,
{
    ScheduleTimeout(Timestamp),
    PreVote {
        pred_hash: BlockHash,
        block_id: BlockId,
    },
    Vote {
        pred_hash: BlockHash,
        block_id: BlockId,
    },
    Propose(Box<Block<Id, Op>>, Id, bool),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum LogRecord {
    Proposal {
        level: i32,
        round: i32,
        timestamp: Timestamp,
    },

    NoPredecessor,
    Predecessor {
        round: i32,
        timestamp: Timestamp,
    },
    TwoTransitionsInRow,
    UnexpectedLevel {
        current: i32,
    },
    UnexpectedRound {
        current: i32,
    },
    SwitchBranch,
    NoSwitchBranch,
    UnexpectedRoundBounded {
        last: i32,
        current: i32,
    },
    UnexpectedRoundFromFuture {
        current: i32,
        block_round: i32,
    },

    AcceptAtEmptyState,
    AcceptAtTransitionState {
        next_level: bool,
    },
    AcceptAtInitializedState {
        next_level: bool,
    },
    PreVote,
    Vote,
    HavePreCertificate {
        payload_round: i32,
    },
    HaveCertificate,

    WillBakeThisLevel {
        round: i32,
        timestamp: Timestamp,
    },
    WillBakeNextLevel {
        round: i32,
        timestamp: Timestamp,
    },

    Proposing {
        level: i32,
        round: i32,
        timestamp: Timestamp,
    },
}

pub enum LogLevel {
    Info,
    Warn,
}

impl LogRecord {
    pub fn level(&self) -> LogLevel {
        use self::LogRecord::*;

        match self {
            NoPredecessor
            | TwoTransitionsInRow
            | UnexpectedLevel { .. }
            | UnexpectedRound { .. }
            | NoSwitchBranch
            | UnexpectedRoundBounded { .. }
            | UnexpectedRoundFromFuture { .. } => LogLevel::Warn,
            _ => LogLevel::Info,
        }
    }
}

impl fmt::Display for LogRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogRecord::Proposal {
                level,
                round,
                timestamp,
            } => {
                write!(f, "Proposal: {level}:{round}, {timestamp}")
            }
            LogRecord::NoPredecessor => {
                write!(f, " .  not empty state, but proposal has no predecessor")
            }
            LogRecord::Predecessor { round, timestamp } => {
                write!(f, " .  predecessor round {round}, {timestamp}")
            }
            LogRecord::TwoTransitionsInRow => {
                write!(f, " .  two block in row are not tenderbake")
            }
            LogRecord::UnexpectedLevel { current } => {
                write!(f, " .  unexpected level, current: {current}")
            }
            LogRecord::UnexpectedRound { current } => {
                write!(f, " .  unexpected round, current: {current}")
            }
            LogRecord::SwitchBranch => write!(f, " .  switch branch"),
            LogRecord::NoSwitchBranch => write!(f, " .  no switch branch"),
            LogRecord::UnexpectedRoundBounded { last, current } => {
                write!(f, " .  unexpected round, last: {last}, current: {current}")
            }
            LogRecord::UnexpectedRoundFromFuture {
                current,
                block_round,
            } => {
                write!(
                    f,
                    " .  unexpected round, block round: {block_round}, current: {current}"
                )
            }
            LogRecord::AcceptAtEmptyState => {
                write!(f, " .  accept at empty state")
            }
            LogRecord::AcceptAtTransitionState { next_level } => {
                write!(f, " .  accept at transition state")?;
                if *next_level {
                    write!(f, ", next level")?;
                }
                Ok(())
            }
            LogRecord::AcceptAtInitializedState { next_level } => {
                write!(f, " .  accept at initialized state")?;
                if *next_level {
                    write!(f, ", next level")?;
                }
                Ok(())
            }
            LogRecord::PreVote => write!(f, " .  cast preendorsement"),
            LogRecord::Vote => write!(f, " .  cast endorsement"),
            LogRecord::HavePreCertificate { payload_round } => {
                write!(f, " .  obtained pre certificate at {payload_round}")
            }
            LogRecord::HaveCertificate => write!(f, " .  obtained certificate"),
            LogRecord::WillBakeThisLevel { round, timestamp } => {
                write!(f, " .  will bake this level, {round}, {timestamp}")
            }
            LogRecord::WillBakeNextLevel { round, timestamp } => {
                write!(f, " .  will bake next level, {round}, {timestamp}")
            }
            LogRecord::Proposing {
                level,
                round,
                timestamp,
            } => {
                write!(f, " .  proposing: {level}:{round}, {timestamp}")
            }
        }
    }
}
