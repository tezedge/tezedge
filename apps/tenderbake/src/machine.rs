// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

use core::{mem, cmp::Ordering, time::Duration};
use alloc::{boxed::Box, vec::Vec};

use arrayvec::ArrayVec;

use super::{
    timestamp::{Timestamp, Timing},
    validator::{Votes, ProposerMap, Validator},
    block::{PayloadHash, BlockHash, PreCertificate, Certificate, Block, TimeHeader, Payload},
    event::{BlockId, Event, Action, LogRecord},
};

/// The state machine. Aims to contain only possible states.
pub struct Machine<Id, Op, const DELAY_MS: u64 = 200>(MachineInner<Id, Op, DELAY_MS>);

/// The state machine. Aims to contain only possible states.
enum MachineInner<Id, Op, const DELAY_MS: u64> {
    // We are only interested in time headers
    Empty,
    // We are only interested in proposals in this state
    // if the proposal of the same level, add its time header to the collection
    // if the proposal of next level, go to next state
    Transition {
        level: i32,
        time_headers: Vec<TimeHeader>,
        timeout_next_level: Option<Timeout<Id>>,
    },
    //
    Initialized(Initialized<Id, Op>),
}

#[derive(Clone)]
struct Timeout<Id> {
    proposer: Id,
    round: i32,
    timestamp: Timestamp,
}

struct Initialized<Id, Op> {
    level: i32,
    // time headers of all possible predecessors
    pred_time_headers: Vec<TimeHeader>,
    // **invariant**
    // should be not empty, last item is the current time header
    this_time_headers: Vec<TimeHeader>,
    pred_hash: BlockHash,
    // block payload
    payload_hash: PayloadHash,
    cer: Option<Certificate<Id, Op>>,
    payload_round: i32,
    operations: Vec<Op>,
    // new block payload
    new_operations: Vec<Op>,
    locked: Option<(i32, PayloadHash)>,
    inner: PreVotesState<Id, Op>,
    timeout_this_level: Option<Timeout<Id>>,
    inner_: VotesState<Id, Op>,
    timeout_next_level: Option<Timeout<Id>>,
}

enum PreVotesState<Id, Op> {
    Collecting {
        incomplete: Votes<Id, Op>,
    },
    Done {
        // **invariant**
        // `pred_time_headers` should contain corresponding time header
        pred_hash: BlockHash,
        operations: Vec<Op>,
        pre_cer: PreCertificate<Id, Op>,
    },
}

enum VotesState<Id, Op> {
    Collecting {
        incomplete: Votes<Id, Op>,
    },
    Done {
        hash: BlockHash,
        cer: Certificate<Id, Op>,
    },
}

impl<Id, Op, const DELAY_MS: u64> Default for Machine<Id, Op, DELAY_MS> {
    fn default() -> Self {
        Machine(MachineInner::Empty)
    }
}

impl<Id, Op, const DELAY_MS: u64> Machine<Id, Op, DELAY_MS>
where
    Id: Clone + Ord,
    Op: Clone,
{
    pub fn handle<T, P>(
        &mut self,
        timing: &T,
        map: &P,
        event: Event<Id, Op>,
    ) -> (ArrayVec<Action<Id, Op>, 2>, ArrayVec<LogRecord, 10>)
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        let mut log = ArrayVec::default();
        let inner = mem::replace(&mut self.0, MachineInner::Empty);
        let (new_state, actions) = match event {
            Event::Proposal(block, now) => {
                log.push(LogRecord::Proposal {
                    level: block.level,
                    round: block.time_header.round,
                    timestamp: block.time_header.timestamp,
                });
                inner.proposal(&mut log, timing, map, *block, now)
            }
            Event::PreVoted(quorum, block_id, validator, now) => {
                inner.pre_voted(&mut log, timing, quorum, block_id, validator, now)
            }
            Event::Voted(quorum, block_id, validator, now) => {
                inner.voted(&mut log, timing, map, quorum, block_id, validator, now)
            }
            Event::Operation(op) => match inner {
                MachineInner::Initialized(mut i) => {
                    i.new_operations.push(op);
                    (MachineInner::Initialized(i), ArrayVec::default())
                }
                z => (z, ArrayVec::default()),
            },
            Event::Timeout => {
                let mut inner = inner;
                let actions = inner.timeout(&mut log);
                (inner, actions)
            }
        };
        self.0 = new_state;
        (actions, log)
    }
}

impl<Id, Op, const DELAY_MS: u64> MachineInner<Id, Op, DELAY_MS>
where
    Id: Clone + Ord,
    Op: Clone,
{
    fn proposal<T, P>(
        self,
        log: &mut ArrayVec<LogRecord, 10>,
        timing: &T,
        map: &P,
        block: Block<Id, Op>,
        now: Timestamp,
    ) -> (Self, ArrayVec<Action<Id, Op>, 2>)
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        match self {
            MachineInner::Empty => {
                let mut actions = ArrayVec::default();
                let level = block.level;
                let this_time_header = block.time_header;
                let timeout_next_level = {
                    let current_round = this_time_header.round_local_coord(timing, now);
                    let slot_next_level = map.proposer(level + 1, current_round);
                    if let Some((round, proposer)) = slot_next_level {
                        let timestamp = this_time_header.timestamp_at_round(timing, round);
                        log.push(LogRecord::WillBakeNextLevel { round, timestamp });
                        actions.push(Action::ScheduleTimeout(timestamp));
                        Some(Timeout {
                            proposer,
                            round,
                            timestamp,
                        })
                    } else {
                        None
                    }
                };
                log.push(LogRecord::AcceptAtEmptyState);
                let new = MachineInner::Transition {
                    level,
                    time_headers: vec![this_time_header],
                    timeout_next_level,
                };
                (new, actions)
            }
            MachineInner::Transition {
                level,
                mut time_headers,
                timeout_next_level,
            } => {
                if block.level == level + 1 {
                    log.push(LogRecord::AcceptAtTransitionState { next_level: true });
                    Self::new_level(time_headers, log, timing, map, block, now)
                } else if block.level == level {
                    log.push(LogRecord::AcceptAtTransitionState { next_level: false });
                    time_headers.push(block.time_header);
                    (
                        MachineInner::Transition {
                            level,
                            time_headers,
                            timeout_next_level,
                        },
                        ArrayVec::default(),
                    )
                } else {
                    log.push(LogRecord::UnexpectedLevel { current: level });
                    // block from far future or from the past, go to empty state
                    MachineInner::Empty.proposal(log, timing, map, block, now)
                }
            }
            MachineInner::Initialized(self_) => {
                if block.level == self_.level + 1 {
                    log.push(LogRecord::AcceptAtInitializedState { next_level: true });
                    Self::new_level(self_.this_time_headers, log, timing, map, block, now)
                } else if block.level == self_.level - 1 {
                    let mut self_ = self_;
                    self_.pred_time_headers.push(block.time_header);
                    (MachineInner::Initialized(self_), ArrayVec::default())
                } else if block.level == self_.level {
                    log.push(LogRecord::AcceptAtInitializedState { next_level: false });
                    let mut block = block;
                    let payload = if let Some(v) = block.payload.take() {
                        v
                    } else {
                        log.push(LogRecord::TwoTransitionsInRow);
                        return MachineInner::Empty.proposal(log, timing, map, block, now);
                    };
                    let block = block;

                    if block.pred_hash != self_.pred_hash {
                        // decide wether should switch or not
                        let switch = match (&self_.inner, &payload.pre_cer) {
                            (PreVotesState::Collecting { .. }, _) => true,
                            (PreVotesState::Done { .. }, None) => false,
                            (PreVotesState::Done { pre_cer, .. }, Some(ref new)) => {
                                match pre_cer.payload_round.cmp(&new.payload_round) {
                                    Ordering::Greater => false,
                                    Ordering::Less => true,
                                    // There is a PQC on two branches with the same round and
                                    // the same level but not the same predecessor : it's
                                    // impossible unless if there was some double-baking. This
                                    // shouldn't happen but do nothing anyway.
                                    Ordering::Equal => false,
                                }
                            }
                        };

                        // ignore the proposal if should not switch
                        if !switch {
                            log.push(LogRecord::NoSwitchBranch);
                            return (MachineInner::Initialized(self_), ArrayVec::default());
                        }

                        log.push(LogRecord::SwitchBranch);
                    }

                    let this_time_header =
                        self_.this_time_headers.last().cloned().expect("invariant");
                    let pred_time_header = match self_
                        .pred_time_headers
                        .iter()
                        .find(|h| h.hash == block.pred_hash)
                    {
                        None => {
                            log.push(LogRecord::NoPredecessor);
                            return MachineInner::Empty.proposal(log, timing, map, block, now);
                        }
                        Some(v) => v.clone(),
                    };

                    let current_round = pred_time_header.round_local_coord(timing, now);

                    let accept_not_pre_vote = this_time_header.round < block.time_header.round
                        && block.time_header.round < current_round;
                    let accept_and_pre_vote = block.time_header.round == current_round
                        && (this_time_header.round != block.time_header.round
                            || payload.hash == self_.payload_hash);
                    if accept_not_pre_vote || accept_and_pre_vote {
                        let mut self_ = self_;
                        self_.pred_hash = block.pred_hash;
                        self_.this_time_headers.push(block.time_header.clone());
                        self_.payload_hash = payload.hash;
                        self_.payload_round = payload.payload_round;
                        self_.operations = payload.operations;
                        self_.new_operations.clear();

                        let mut actions = ArrayVec::default();

                        let new_payload_round =
                            payload.pre_cer.as_ref().map(|new| new.payload_round);

                        let best_pred_hash = match (&self_.inner, payload.pre_cer) {
                            (PreVotesState::Done { ref pre_cer, .. }, Some(new))
                                if new.payload_round > pre_cer.payload_round =>
                            {
                                log.push(LogRecord::HavePreCertificate {
                                    payload_round: new.payload_round,
                                });
                                self_.inner = PreVotesState::Done {
                                    pred_hash: self_.pred_hash.clone(),
                                    operations: self_.operations.clone(),
                                    pre_cer: new,
                                };
                                &self_.pred_hash
                            }
                            (PreVotesState::Done { ref pred_hash, .. }, _) => pred_hash,
                            (PreVotesState::Collecting { .. }, Some(new)) => {
                                log.push(LogRecord::HavePreCertificate {
                                    payload_round: new.payload_round,
                                });
                                self_.inner = PreVotesState::Done {
                                    pred_hash: self_.pred_hash.clone(),
                                    operations: self_.operations.clone(),
                                    pre_cer: new,
                                };
                                &self_.pred_hash
                            }
                            _ => &self_.pred_hash,
                        };
                        // TODO: not sure this is needed,
                        // can we use `pred_time_header` already defined above?
                        let pred_time_header = self_
                            .pred_time_headers
                            .iter()
                            .find(|h| h.hash.eq(best_pred_hash))
                            .expect("invariant");

                        self_.timeout_next_level = match &self_.inner_ {
                            VotesState::Done { ref hash, .. } => {
                                let this_time_header = self_
                                    .this_time_headers
                                    .iter()
                                    .find(|h| h.hash.eq(hash))
                                    .expect("invariant");
                                let current_round = this_time_header.round_local_coord(timing, now);
                                let slot_next_level = map.proposer(self_.level + 1, current_round);
                                if let Some((round, proposer)) = slot_next_level {
                                    let timestamp =
                                        this_time_header.timestamp_at_round(timing, round);
                                    log.push(LogRecord::WillBakeNextLevel { round, timestamp });
                                    Some(Timeout {
                                        proposer,
                                        round,
                                        timestamp,
                                    })
                                } else {
                                    None
                                }
                            }
                            VotesState::Collecting { .. } => None,
                        };

                        self_.timeout_this_level = {
                            // TODO: not sure this is needed,
                            let current_round = pred_time_header.round_local_coord(timing, now);
                            let slot_this_level = map.proposer(block.level, current_round + 1);
                            if let Some((round, proposer)) = slot_this_level {
                                let timestamp = pred_time_header.timestamp_at_round(timing, round);
                                log.push(LogRecord::WillBakeThisLevel { round, timestamp });
                                Some(Timeout {
                                    proposer,
                                    round,
                                    timestamp,
                                })
                            } else {
                                None
                            }
                        };

                        let delay = Duration::from_millis(DELAY_MS);
                        let t = match (&self_.timeout_this_level, &self_.timeout_next_level) {
                            (Some(ref this), Some(ref next)) => {
                                if this.timestamp < next.timestamp {
                                    Some(this.timestamp + delay)
                                } else {
                                    Some(next.timestamp)
                                }
                            }
                            (Some(ref this), None) => Some(this.timestamp + delay),
                            (None, Some(ref next)) => Some(next.timestamp),
                            _ => None,
                        };
                        actions.extend(t.map(Action::ScheduleTimeout).into_iter());

                        let will_pre_vote = accept_and_pre_vote
                            && match &self_.locked {
                                Some((ref locked_round, ref locked_hash)) => {
                                    new_payload_round.unwrap_or(-1) >= *locked_round
                                        || locked_hash.eq(&self_.payload_hash)
                                }
                                None => true,
                            };
                        if will_pre_vote {
                            log.push(LogRecord::PreVote);
                            actions.push(Action::PreVote {
                                pred_hash: self_.pred_hash.clone(),
                                block_id: BlockId {
                                    level: block.level,
                                    round: current_round,
                                    payload_hash: self_.payload_hash.clone(),
                                },
                            })
                        }

                        (MachineInner::Initialized(self_), actions)
                    } else {
                        log.push(LogRecord::UnexpectedRoundBounded {
                            last: this_time_header.round,
                            current: current_round,
                        });
                        (MachineInner::Initialized(self_), ArrayVec::default())
                    }
                } else {
                    log.push(LogRecord::UnexpectedLevel {
                        current: self_.level,
                    });
                    // block from far future or from the past, go to empty state
                    MachineInner::Empty.proposal(log, timing, map, block, now)
                }
            }
        }
    }

    fn new_level<T, P>(
        pred_time_headers: Vec<TimeHeader>,
        log: &mut ArrayVec<LogRecord, 10>,
        timing: &T,
        map: &P,
        block: Block<Id, Op>,
        now: Timestamp,
    ) -> (Self, ArrayVec<Action<Id, Op>, 2>)
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        let pred_time_header = match pred_time_headers.iter().find(|h| h.hash == block.pred_hash) {
            None => {
                log.push(LogRecord::NoPredecessor);
                return MachineInner::Empty.proposal(log, timing, map, block, now);
            }
            Some(v) => v.clone(),
        };

        let payload = if let Some(v) = block.payload {
            v
        } else {
            log.push(LogRecord::TwoTransitionsInRow);
            return MachineInner::Empty.proposal(log, timing, map, block, now);
        };

        let current_round = pred_time_header.round_local_coord(timing, now);
        if current_round < block.time_header.round {
            // proposal from future, ignore
            log.push(LogRecord::UnexpectedRound {
                current: current_round,
            });
            let level = block.level - 1;
            return (
                MachineInner::Transition {
                    level,
                    time_headers: pred_time_headers,
                    timeout_next_level: None, // TODO: reuse code
                },
                ArrayVec::default(),
            );
        }

        let mut actions = ArrayVec::default();
        if current_round == block.time_header.round {
            log.push(LogRecord::PreVote);
            actions.push(Action::PreVote {
                pred_hash: block.pred_hash.clone(),
                block_id: BlockId {
                    level: block.level,
                    round: current_round,
                    payload_hash: payload.hash.clone(),
                },
            })
        }

        let inner = if let Some(pre_cer) = payload.pre_cer {
            log.push(LogRecord::HavePreCertificate {
                payload_round: pre_cer.payload_round,
            });
            PreVotesState::Done {
                pred_hash: block.pred_hash.clone(),
                operations: payload.operations.clone(),
                pre_cer,
            }
        } else {
            PreVotesState::Collecting {
                incomplete: Votes::default(),
            }
        };

        let timeout_this_level = {
            let slot_this_level = map.proposer(block.level, current_round + 1);
            if let Some((round, proposer)) = slot_this_level {
                let timestamp = pred_time_header.timestamp_at_round(timing, round);
                log.push(LogRecord::WillBakeThisLevel { round, timestamp });
                let delay = Duration::from_millis(DELAY_MS);
                actions.push(Action::ScheduleTimeout(timestamp + delay));
                Some(Timeout {
                    proposer,
                    round,
                    timestamp,
                })
            } else {
                None
            }
        };

        (
            MachineInner::Initialized(Initialized {
                level: block.level,
                pred_time_headers,
                this_time_headers: vec![block.time_header],
                pred_hash: block.pred_hash,
                payload_hash: payload.hash,
                cer: payload.cer,
                payload_round: payload.payload_round,
                operations: payload.operations,
                new_operations: vec![],
                locked: None,
                inner,
                timeout_this_level,
                inner_: VotesState::Collecting {
                    incomplete: Votes::default(),
                },
                timeout_next_level: None,
            }),
            actions,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn pre_voted<T>(
        self,
        log: &mut ArrayVec<LogRecord, 10>,
        timing: &T,
        quorum: u32,
        block_id: BlockId,
        validator: Validator<Id, Op>,
        now: Timestamp,
    ) -> (Self, ArrayVec<Action<Id, Op>, 2>)
    where
        T: Timing,
    {
        let self_ = match self {
            MachineInner::Initialized(v) => v,
            _ => return (self, ArrayVec::default()),
        };

        let pred_time_header = self_
            .pred_time_headers
            .iter()
            .find(|h| h.hash == self_.pred_hash)
            .expect("invariant");
        let current_round = pred_time_header.round_local_coord(timing, now);

        if block_id.level != self_.level
            || block_id.payload_hash != self_.payload_hash.clone()
            || block_id.round != self_.this_time_headers.last().expect("invariant").round
            || block_id.round != current_round
        {
            return (MachineInner::Initialized(self_), ArrayVec::default());
        }

        let mut self_ = self_;
        let votes = match &mut self_.inner {
            PreVotesState::Collecting { ref mut incomplete } => incomplete,
            _ => return (MachineInner::Initialized(self_), ArrayVec::default()),
        };

        *votes += validator;

        let mut actions = ArrayVec::default();
        if votes.power >= quorum {
            if let Some((ref round, ref payload_hash)) = &self_.locked {
                if block_id.round.eq(round) && block_id.payload_hash.eq(payload_hash) {
                    return (MachineInner::Initialized(self_), ArrayVec::default());
                }
            }

            log.push(LogRecord::HavePreCertificate {
                payload_round: current_round,
            });
            log.push(LogRecord::Vote);
            self_.locked = Some((block_id.round, block_id.payload_hash.clone()));
            self_.inner = PreVotesState::Done {
                pred_hash: self_.pred_hash.clone(),
                operations: self_.operations.clone(),
                pre_cer: PreCertificate {
                    payload_hash: block_id.payload_hash.clone(),
                    payload_round: block_id.round,
                    votes: mem::take(votes),
                },
            };
            actions.push(Action::Vote {
                pred_hash: self_.pred_hash.clone(),
                block_id,
            });
        }
        (MachineInner::Initialized(self_), actions)
    }

    #[allow(clippy::too_many_arguments)]
    fn voted<T, P>(
        self,
        log: &mut ArrayVec<LogRecord, 10>,
        timing: &T,
        map: &P,
        quorum: u32,
        block_id: BlockId,
        validator: Validator<Id, Op>,
        now: Timestamp,
    ) -> (Self, ArrayVec<Action<Id, Op>, 2>)
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        let self_ = match self {
            MachineInner::Initialized(v) => v,
            _ => return (self, ArrayVec::default()),
        };

        let pred_time_header = self_
            .pred_time_headers
            .iter()
            .find(|h| h.hash == self_.pred_hash)
            .expect("invariant");
        let current_round = pred_time_header.round_local_coord(timing, now);

        if block_id.level != self_.level
            || block_id.payload_hash != self_.payload_hash.clone()
            || block_id.round != self_.this_time_headers.last().expect("invariant").round
            || block_id.round != current_round
        {
            return (MachineInner::Initialized(self_), ArrayVec::default());
        }

        let mut self_ = self_;
        let votes = match &mut self_.inner_ {
            VotesState::Collecting { incomplete } => incomplete,
            _ => return (MachineInner::Initialized(self_), ArrayVec::default()),
        };

        *votes += validator;

        let mut actions = ArrayVec::default();
        if votes.power >= quorum {
            log.push(LogRecord::HaveCertificate);
            let this_time_header = self_.this_time_headers.last().cloned().expect("invariant");
            self_.inner_ = VotesState::Done {
                hash: this_time_header.hash.clone(),
                cer: Certificate {
                    votes: mem::take(votes),
                },
            };
            self_.timeout_next_level = {
                let current_round = this_time_header.round_local_coord(timing, now);
                let slot_next_level = map.proposer(self_.level + 1, current_round);
                if let Some((round, proposer)) = slot_next_level {
                    let timestamp = this_time_header.timestamp_at_round(timing, round);
                    log.push(LogRecord::WillBakeNextLevel { round, timestamp });
                    match &self_.timeout_this_level {
                        Some(ref t) if t.timestamp < timestamp => (),
                        _ => actions.push(Action::ScheduleTimeout(timestamp)),
                    }
                    Some(Timeout {
                        proposer,
                        round,
                        timestamp,
                    })
                } else {
                    None
                }
            };
        }
        (MachineInner::Initialized(self_), actions)
    }

    fn timeout(&mut self, log: &mut ArrayVec<LogRecord, 10>) -> ArrayVec<Action<Id, Op>, 2> {
        match self {
            MachineInner::Transition {
                level,
                time_headers,
                timeout_next_level,
            } => {
                if let Some(Timeout {
                    proposer,
                    round,
                    timestamp,
                }) = timeout_next_level.take()
                {
                    let mut actions = ArrayVec::default();
                    let new_block = Block {
                        pred_hash: time_headers.last().cloned().expect("invariant").hash,
                        level: *level + 1,
                        time_header: TimeHeader {
                            hash: BlockHash([0; 32]),
                            round,
                            timestamp,
                        },
                        payload: Some(Payload {
                            hash: PayloadHash([0; 32]),
                            payload_round: round,
                            pre_cer: None,
                            cer: None,
                            operations: vec![],
                        }),
                    };
                    actions.push(Action::Propose(Box::new(new_block), proposer, false));
                    actions
                } else {
                    ArrayVec::default()
                }
            }
            MachineInner::Initialized(ref mut self_) => {
                let (
                    this,
                    Timeout {
                        proposer,
                        round,
                        timestamp,
                    },
                ) = match (&mut self_.timeout_this_level, &mut self_.timeout_next_level) {
                    (Some(ref mut this), Some(ref mut next)) => {
                        if this.timestamp < next.timestamp {
                            (true, this.clone())
                        } else {
                            (false, next.clone())
                        }
                    }
                    (Some(ref mut this), None) => (true, this.clone()),
                    (None, Some(ref mut next)) => (false, next.clone()),
                    (None, None) => return ArrayVec::default(),
                };
                let time_header = TimeHeader {
                    hash: BlockHash([0; 32]),
                    round,
                    timestamp,
                };
                let new_block = if this {
                    self_.timeout_this_level = None;
                    match &self_.inner {
                        PreVotesState::Done {
                            pred_hash,
                            operations,
                            pre_cer,
                        } => Block {
                            pred_hash: pred_hash.clone(),
                            level: self_.level,
                            time_header,
                            payload: Some(Payload {
                                hash: pre_cer.payload_hash.clone(),
                                payload_round: self_.payload_round,
                                pre_cer: Some(pre_cer.clone()),
                                cer: self_.cer.clone(),
                                operations: operations.clone(),
                            }),
                        },
                        PreVotesState::Collecting { .. } => Block {
                            pred_hash: self_.pred_hash.clone(),
                            level: self_.level,
                            time_header,
                            payload: Some(Payload {
                                hash: PayloadHash([0; 32]),
                                payload_round: 0,
                                pre_cer: None,
                                cer: self_.cer.clone(),
                                operations: self_.operations.clone(),
                            }),
                        },
                    }
                } else {
                    self_.timeout_next_level = None;
                    match &self_.inner_ {
                        VotesState::Done { cer, hash } => Block {
                            pred_hash: hash.clone(),
                            level: self_.level + 1,
                            time_header,
                            payload: Some(Payload {
                                hash: PayloadHash([0; 32]),
                                payload_round: round,
                                pre_cer: None,
                                cer: Some(cer.clone()),
                                operations: self_.new_operations.clone(),
                            }),
                        },
                        _ => return ArrayVec::default(),
                    }
                };
                let mut actions = ArrayVec::new();
                log.push(LogRecord::Proposing {
                    level: new_block.level,
                    round: new_block.time_header.round,
                    timestamp: new_block.time_header.timestamp,
                });
                actions.push(Action::Propose(Box::new(new_block), proposer, this));
                let delay = Duration::from_millis(DELAY_MS);
                let t = match (&self_.timeout_this_level, &self_.timeout_next_level) {
                    (Some(ref this), None) => Some(this.timestamp + delay),
                    (None, Some(ref next)) => Some(next.timestamp),
                    _ => None,
                };
                actions.extend(t.map(Action::ScheduleTimeout));
                actions
            }
            _ => ArrayVec::default(),
        }
    }
}
