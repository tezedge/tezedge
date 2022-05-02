// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{mem, cmp::Ordering, time::Duration};
use alloc::{boxed::Box, vec::Vec, collections::BTreeMap};

// arrayvec is unsafe inside
// use arrayvec::ArrayVec;
// bounded array
type ArrayVec<T> = Vec<T>;

use super::{
    timestamp::{Timestamp, Timing},
    validator::{Votes, ProposerMap, Validator},
    block::{PayloadHash, BlockHash, PreCertificate, Certificate, Block, Payload},
    timeout::{Config, Timeout, TimeHeader},
    event::{BlockId, Event, Action, LogRecord},
};

/// The state machine. Aims to contain only possible states.
pub struct Machine<Id, Op, const DELAY_MS: u64> {
    inner: Option<Result<Initialized<Id, Op, DELAY_MS>, Transition<Id>>>,
}

struct Pair<L, Id, Op>(L, ArrayVec<Action<Id, Op>>);

impl<L, Id, Op> Pair<L, Id, Op> {
    fn map_left<F, Lp>(self, f: F) -> Pair<Lp, Id, Op>
    where
        F: FnOnce(L) -> Lp,
    {
        Pair(f(self.0), self.1)
    }
}

// We are only interested in proposals in this state
// if the proposal of the same level, add its time header to the collection
// if the proposal of next level, go to next state
struct Transition<Id> {
    level: i32,
    hash: BlockHash,
    time_headers: BTreeMap<BlockHash, TimeHeader<false>>,
    timeout_next_level: Option<Timeout<Id>>,
}

struct Initialized<Id, Op, const DELAY_MS: u64> {
    level: i32,
    // time headers of all possible predecessors
    pred_time_headers: BTreeMap<BlockHash, TimeHeader<true>>,
    this_time_headers: BTreeMap<BlockHash, TimeHeader<false>>,
    hash: BlockHash,
    // current
    this_time_header: TimeHeader<false>,
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
        round: i32,
        hash: BlockHash,
        cer: Certificate<Id, Op>,
    },
}

impl<Id, Op, const DELAY_MS: u64> Default for Machine<Id, Op, DELAY_MS> {
    fn default() -> Self {
        Machine { inner: None }
    }
}

impl<Id, Op, const DELAY_MS: u64> Machine<Id, Op, DELAY_MS>
where
    Id: Clone + Ord,
    Op: Clone,
{
    pub fn handle<T, P>(
        &mut self,
        config: &Config<T, P>,
        event: Event<Id, Op>,
    ) -> (ArrayVec<Action<Id, Op>>, ArrayVec<LogRecord>)
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        let mut log = ArrayVec::default();
        let inner = self.inner.take();
        let Pair(new_state, actions) = match event {
            Event::Proposal(block, now) => {
                log.push(LogRecord::Proposal {
                    level: block.level,
                    round: block.time_header.round,
                    timestamp: block.time_header.timestamp,
                });
                let new = match inner {
                    None => Transition::next_level(&mut log, config, *block, now).map_left(Err),
                    Some(Err(self_)) => {
                        if block.level == self_.level + 1 {
                            log.push(LogRecord::AcceptAtTransitionState { next_level: true });
                            let time_headers = self_
                                .time_headers
                                .into_iter()
                                .map(|(k, v)| (k, TimeHeader::into_prev(v)))
                                .collect();
                            Initialized::next_level(time_headers, &mut log, config, *block, now)
                        } else if block.level == self_.level {
                            log.push(LogRecord::AcceptAtTransitionState { next_level: false });
                            self_.next_round(*block).map_left(Err)
                        } else {
                            log.push(LogRecord::UnexpectedLevel {
                                current: self_.level,
                            });
                            // block from far future or from the past, go to empty state
                            Transition::next_level(&mut log, config, *block, now).map_left(Err)
                        }
                    }
                    Some(Ok(self_)) => {
                        if block.level == self_.level + 1 {
                            log.push(LogRecord::AcceptAtInitializedState { next_level: true });
                            let mut time_headers = self_.this_time_headers;
                            time_headers.insert(self_.hash.clone(), self_.this_time_header);
                            let time_headers = time_headers
                                .into_iter()
                                .map(|(k, v)| (k, TimeHeader::into_prev(v)))
                                .collect();
                            Initialized::next_level(time_headers, &mut log, config, *block, now)
                        } else if block.level == self_.level - 1 {
                            let mut self_ = self_;
                            self_
                                .pred_time_headers
                                .insert(block.hash, block.time_header.into_prev());
                            Pair(Ok(self_), ArrayVec::default())
                        } else if block.level == self_.level {
                            log.push(LogRecord::AcceptAtInitializedState { next_level: false });
                            Initialized::next_round(self_, &mut log, config, *block, now)
                        } else {
                            log.push(LogRecord::UnexpectedLevel {
                                current: self_.level,
                            });
                            // block from far future or from the past, go to empty state
                            Transition::next_level(&mut log, config, *block, now).map_left(Err)
                        }
                    }
                };
                new.map_left(Some)
            }
            Event::PreVoted(block_id, validator, now) => match inner {
                Some(Ok(m)) => m
                    .pre_voted(&mut log, config, block_id, validator, now)
                    .map_left(Ok)
                    .map_left(Some),
                x => Pair(x, ArrayVec::default()),
            },
            Event::Voted(block_id, validator, now) => match inner {
                Some(Ok(m)) => m
                    .voted(&mut log, config, block_id, validator, now)
                    .map_left(Ok)
                    .map_left(Some),
                x => Pair(x, ArrayVec::default()),
            },
            Event::Operation(op) => match inner {
                Some(Ok(mut m)) => {
                    m.new_operations.push(op);
                    Pair(Some(Ok(m)), ArrayVec::default())
                }
                x => Pair(x, ArrayVec::default()),
            },
            Event::Timeout => match inner {
                None => Pair(None, ArrayVec::default()),
                Some(Ok(mut m)) => {
                    let actions = m.timeout(&mut log);
                    Pair(Some(Ok(m)), actions)
                }
                Some(Err(mut m)) => {
                    let actions = m.timeout(&mut log);
                    Pair(Some(Err(m)), actions)
                }
            },
        };
        self.inner = new_state;
        (actions, log)
    }

    pub fn elected_block(&self) -> Option<BlockHash> {
        let initialized = self.inner.as_ref()?.as_ref().ok()?;
        match &initialized.inner_ {
            VotesState::Done { hash, .. } => Some(hash.clone()),
            VotesState::Collecting { .. } => None,
        }
    }
}

impl<Id> Transition<Id>
where
    Id: Clone + Ord,
{
    fn next_level<T, P, Op>(
        log: &mut ArrayVec<LogRecord>,
        config: &Config<T, P>,
        block: Block<Id, Op>,
        now: Timestamp,
    ) -> Pair<Self, Id, Op>
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        log.push(LogRecord::AcceptAtEmptyState);
        let mut actions = ArrayVec::default();
        // bake only if it is transition block (payload is none)
        let timeout_next_level = if block.payload.is_none() {
            block.time_header.calculate(config, now, block.level)
        } else {
            None
        };
        actions.extend(
            timeout_next_level
                .as_ref()
                .map(|t| Action::ScheduleTimeout(t.timestamp)),
        );
        let new = Transition {
            hash: block.hash.clone(),
            level: block.level,
            time_headers: {
                let mut m = BTreeMap::new();
                m.insert(block.hash, block.time_header);
                m
            },
            timeout_next_level,
        };
        Pair(new, actions)
    }

    fn next_round<Op>(mut self, block: Block<Id, Op>) -> Pair<Self, Id, Op> {
        self.time_headers.insert(block.hash, block.time_header);
        Pair(self, ArrayVec::default())
    }
}

impl<Id, Op, const DELAY_MS: u64> Initialized<Id, Op, DELAY_MS>
where
    Id: Ord + Clone,
    Op: Clone,
{
    fn next_level<T, P>(
        pred_time_headers: BTreeMap<BlockHash, TimeHeader<true>>,
        log: &mut ArrayVec<LogRecord>,
        config: &Config<T, P>,
        block: Block<Id, Op>,
        now: Timestamp,
    ) -> Pair<Result<Self, Transition<Id>>, Id, Op>
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        let pred_time_header = match pred_time_headers.get(&block.pred_hash) {
            None => {
                log.push(LogRecord::NoPredecessor);
                return Transition::next_level(log, config, block, now).map_left(Err);
            }
            Some(v) => {
                log.push(LogRecord::Predecessor {
                    round: v.round,
                    timestamp: v.timestamp,
                });
                v.clone()
            }
        };

        let payload = if let Some(v) = block.payload {
            v
        } else {
            log.push(LogRecord::TwoTransitionsInRow);
            return Transition::next_level(log, config, block, now).map_left(Err);
        };

        let current_round = pred_time_header.round_local_coord(&config.timing, now);

        // block from future, let's accept it and write a warning
        let current_round = if current_round < block.time_header.round {
            log.push(LogRecord::UnexpectedRoundFromFuture {
                current: current_round,
                block_round: block.time_header.round,
            });
            block.time_header.round
        } else {
            current_round
        };

        if current_round < block.time_header.round {
            // proposal from future, ignore
            log.push(LogRecord::UnexpectedRound {
                current: current_round,
            });
            let level = block.level - 1;
            return Pair(
                Err(Transition {
                    hash: block.hash,
                    level,
                    time_headers: pred_time_headers
                        .into_iter()
                        .map(|(k, v)| (k, TimeHeader::into_this(v)))
                        .collect(),
                    timeout_next_level: None, // TODO: reuse code
                }),
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

        let timeout_this_level = pred_time_header.calculate(config, now, block.level);
        let delay = Duration::from_millis(DELAY_MS);
        actions.extend(
            timeout_this_level
                .as_ref()
                .map(|t| Action::ScheduleTimeout(t.timestamp + delay)),
        );

        Pair(
            Ok(Initialized {
                level: block.level,
                pred_time_headers,
                this_time_headers: BTreeMap::default(),
                hash: block.hash,
                this_time_header: block.time_header,
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

    fn next_round<T, P>(
        self_: Initialized<Id, Op, DELAY_MS>,
        log: &mut ArrayVec<LogRecord>,
        config: &Config<T, P>,
        block: Block<Id, Op>,
        now: Timestamp,
    ) -> Pair<Result<Self, Transition<Id>>, Id, Op>
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        let mut block = block;
        let payload = if let Some(v) = block.payload.take() {
            v
        } else {
            log.push(LogRecord::TwoTransitionsInRow);
            return Transition::next_level(log, config, block, now).map_left(Err);
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
                return Pair(Ok(self_), ArrayVec::default());
            }

            log.push(LogRecord::SwitchBranch);
        }

        let pred_time_header = match self_.pred_time_headers.get(&block.pred_hash) {
            None => {
                log.push(LogRecord::NoPredecessor);
                return Transition::next_level(log, config, block, now).map_left(Err);
            }
            Some(v) => {
                log.push(LogRecord::Predecessor {
                    round: v.round,
                    timestamp: v.timestamp,
                });
                v.clone()
            }
        };

        let current_round = pred_time_header.round_local_coord(&config.timing, now);

        // block from future, let's accept it and write a warning
        let current_round = if current_round < block.time_header.round {
            log.push(LogRecord::UnexpectedRoundFromFuture {
                current: current_round,
                block_round: block.time_header.round,
            });
            block.time_header.round
        } else {
            current_round
        };

        let new_round = block.time_header.round;
        let accept_not_pre_vote =
            self_.this_time_header.round < new_round && new_round < current_round;
        let accept_and_pre_vote = new_round == current_round
            && (self_.this_time_header.round != new_round || payload.hash == self_.payload_hash);
        if accept_not_pre_vote || accept_and_pre_vote {
            let mut self_ = self_;
            self_.pred_hash = block.pred_hash;
            let hdr = mem::replace(&mut self_.this_time_header, block.time_header.clone());
            let hash = mem::replace(&mut self_.hash, block.hash.clone());
            self_.this_time_headers.insert(hash, hdr);
            self_.payload_hash = payload.hash;
            self_.payload_round = payload.payload_round;
            self_.operations = payload.operations;
            self_.cer = payload.cer;
            // self_.new_operations.clear();

            let mut actions = ArrayVec::default();

            let new_payload_round = payload.pre_cer.as_ref().map(|new| new.payload_round);

            match (&self_.inner, payload.pre_cer) {
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
                }
                (PreVotesState::Done { .. }, _) => (),
                (PreVotesState::Collecting { .. }, Some(new)) => {
                    log.push(LogRecord::HavePreCertificate {
                        payload_round: new.payload_round,
                    });
                    self_.inner = PreVotesState::Done {
                        pred_hash: self_.pred_hash.clone(),
                        operations: self_.operations.clone(),
                        pre_cer: new,
                    };
                }
                _ => (),
            };
            if let VotesState::Collecting { ref mut incomplete } = &mut self_.inner_ {
                *incomplete = Votes::default();
            }

            self_.timeout_next_level = match &self_.inner_ {
                VotesState::Done { ref hash, .. } => self_
                    .this_time_headers
                    .get(hash)
                    .expect("invariant")
                    .calculate(config, now, self_.level),
                VotesState::Collecting { .. } => None,
            };
            self_.timeout_this_level = pred_time_header.calculate(config, now, block.level);

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
            if let PreVotesState::Done { .. } = &self_.inner {
                log.push(LogRecord::Vote);
                actions.push(Action::Vote {
                    pred_hash: self_.pred_hash.clone(),
                    block_id: BlockId {
                        level: block.level,
                        round: current_round,
                        payload_hash: self_.payload_hash.clone(),
                    },
                })
            }

            Pair(Ok(self_), actions)
        } else {
            log.push(LogRecord::UnexpectedRoundBounded {
                last: self_.this_time_header.round,
                current: current_round,
            });
            Pair(Ok(self_), ArrayVec::default())
        }
    }

    fn pre_voted<T, P>(
        self,
        log: &mut ArrayVec<LogRecord>,
        config: &Config<T, P>,
        block_id: BlockId,
        validator: Validator<Id, Op>,
        now: Timestamp,
    ) -> Pair<Self, Id, Op>
    where
        T: Timing,
    {
        let pred_time_header = self
            .pred_time_headers
            .get(&self.pred_hash)
            .expect("invariant");
        let current_round = pred_time_header.round_local_coord(&config.timing, now);

        if block_id.level != self.level
            || block_id.payload_hash != self.payload_hash.clone()
            || block_id.round != self.this_time_header.round
            || block_id.round < current_round
        {
            return Pair(self, ArrayVec::default());
        }

        let mut self_ = self;
        let votes = match &mut self_.inner {
            PreVotesState::Collecting { ref mut incomplete } => incomplete,
            PreVotesState::Done {
                ref mut pre_cer, ..
            } => {
                pre_cer.votes += validator;
                return Pair(self_, ArrayVec::default());
            }
        };

        *votes += validator;

        let mut actions = ArrayVec::default();
        if votes.power >= config.quorum {
            if let Some((ref round, ref payload_hash)) = &self_.locked {
                if block_id.round.eq(round) && block_id.payload_hash.eq(payload_hash) {
                    return Pair(self_, ArrayVec::default());
                }
            }

            log.push(LogRecord::HavePreCertificate {
                payload_round: current_round,
            });
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
            log.push(LogRecord::Vote);
            actions.push(Action::Vote {
                pred_hash: self_.pred_hash.clone(),
                block_id,
            });
        }
        Pair(self_, actions)
    }

    fn voted<T, P>(
        mut self,
        log: &mut ArrayVec<LogRecord>,
        config: &Config<T, P>,
        block_id: BlockId,
        validator: Validator<Id, Op>,
        now: Timestamp,
    ) -> Pair<Self, Id, Op>
    where
        T: Timing,
        P: ProposerMap<Id = Id>,
    {
        let pred_time_header = self
            .pred_time_headers
            .get(&self.pred_hash)
            .expect("invariant");
        let current_round = pred_time_header.round_local_coord(&config.timing, now);

        // if we have elected block, and the vote is late, let's include it anyway
        if let VotesState::Done {
            ref round,
            ref mut cer,
            ..
        } = &mut self.inner_
        {
            if round.eq(&block_id.round) {
                cer.votes += validator;
            }
            return Pair(self, ArrayVec::default());
        }

        if block_id.level != self.level
            || block_id.payload_hash != self.payload_hash.clone()
            || block_id.round != self.this_time_header.round
            || block_id.round < current_round
        {
            return Pair(self, ArrayVec::default());
        }

        let votes = match &mut self.inner_ {
            VotesState::Collecting { ref mut incomplete } => incomplete,
            VotesState::Done {
                ref mut cer,
                ref hash,
                ..
            } => {
                if self.hash.eq(hash) {
                    cer.votes += validator;
                }
                return Pair(self, ArrayVec::default());
            }
        };

        *votes += validator;

        let mut actions = ArrayVec::default();
        if votes.power >= config.quorum {
            log.push(LogRecord::HaveCertificate);
            self.inner_ = VotesState::Done {
                hash: self.hash.clone(),
                round: block_id.round,
                cer: Certificate {
                    votes: mem::take(votes),
                },
            };
            self.timeout_next_level = self.this_time_header.calculate(config, now, self.level);

            if let Some(ref n) = &self.timeout_next_level {
                let timestamp = n.timestamp;
                match &self.timeout_this_level {
                    Some(ref t) if t.timestamp < timestamp => (),
                    _ => actions.push(Action::ScheduleTimeout(timestamp)),
                }
            }
        }
        Pair(self, actions)
    }
}

impl<Id> Transition<Id> {
    fn timeout<Op>(&mut self, log: &mut ArrayVec<LogRecord>) -> ArrayVec<Action<Id, Op>> {
        if let Some(Timeout {
            proposer,
            round,
            timestamp,
        }) = self.timeout_next_level.take()
        {
            let mut actions = ArrayVec::default();
            let new_block = Block {
                pred_hash: self.hash.clone(),
                hash: BlockHash([0; 32]),
                level: self.level + 1,
                time_header: TimeHeader { round, timestamp },
                payload: Some(Payload {
                    hash: PayloadHash([0; 32]),
                    payload_round: round,
                    pre_cer: None,
                    cer: None,
                    operations: vec![],
                }),
            };
            log.push(LogRecord::Proposing {
                level: new_block.level,
                round: new_block.time_header.round,
                timestamp: new_block.time_header.timestamp,
            });
            actions.push(Action::Propose(Box::new(new_block), proposer, false));
            actions
        } else {
            ArrayVec::default()
        }
    }
}

impl<Id, Op, const DELAY_MS: u64> Initialized<Id, Op, DELAY_MS>
where
    Id: Clone + Ord,
    Op: Clone,
{
    fn timeout(&mut self, log: &mut ArrayVec<LogRecord>) -> ArrayVec<Action<Id, Op>> {
        let (
            this,
            Timeout {
                proposer,
                round,
                timestamp,
            },
        ) = match (&mut self.timeout_this_level, &mut self.timeout_next_level) {
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
        let time_header = TimeHeader { round, timestamp };
        let new_block = if this {
            self.timeout_this_level = None;
            match &self.inner {
                PreVotesState::Done {
                    pred_hash,
                    operations,
                    pre_cer,
                } => Block {
                    pred_hash: pred_hash.clone(),
                    hash: BlockHash([0; 32]),
                    level: self.level,
                    time_header,
                    payload: Some(Payload {
                        hash: pre_cer.payload_hash.clone(),
                        payload_round: self.payload_round,
                        pre_cer: Some(pre_cer.clone()),
                        cer: self.cer.clone(),
                        operations: operations.clone(),
                    }),
                },
                PreVotesState::Collecting { .. } => Block {
                    pred_hash: self.pred_hash.clone(),
                    hash: BlockHash([0; 32]),
                    level: self.level,
                    time_header,
                    payload: Some(Payload {
                        hash: PayloadHash([0; 32]),
                        payload_round: 0,
                        pre_cer: None,
                        cer: self.cer.clone(),
                        operations: self.operations.clone(),
                    }),
                },
            }
        } else {
            self.timeout_next_level = None;
            match &self.inner_ {
                VotesState::Done { cer, hash, .. } => Block {
                    pred_hash: hash.clone(),
                    hash: BlockHash([0; 32]),
                    level: self.level + 1,
                    time_header,
                    payload: Some(Payload {
                        hash: PayloadHash([0; 32]),
                        payload_round: round,
                        pre_cer: None,
                        cer: Some(cer.clone()),
                        operations: self.new_operations.clone(),
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
        let t = match (&self.timeout_this_level, &self.timeout_next_level) {
            (Some(ref this), None) => Some(this.timestamp + delay),
            (None, Some(ref next)) => Some(next.timestamp),
            _ => None,
        };
        actions.extend(t.map(Action::ScheduleTimeout));
        actions
    }
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use alloc::boxed::Box;

    use crate::{
        Machine, Config, TimingLinearGrow, ProposerMap, Event, Block, BlockHash, TimeHeader,
        Timestamp, Payload, PayloadHash, BlockId, Validator, Action,
    };

    #[test]
    fn determine_round() {
        let mut machine = Machine::<u8, u8, 200>::default();

        struct Map;

        impl ProposerMap for Map {
            type Id = u8;

            fn proposer(&self, level: i32, round: i32) -> Option<(i32, Self::Id)> {
                let _ = (level, round);
                None
            }
        }

        let config = Config {
            timing: TimingLinearGrow {
                minimal_block_delay: Duration::from_secs(15),
                delay_increment_per_round: Duration::from_secs(5),
            },
            map: Map,
            quorum: 256,
        };

        let mut block = Block {
            pred_hash: BlockHash([0; 32]),
            hash: BlockHash([1; 32]),
            level: 5,
            time_header: TimeHeader {
                round: 0,
                timestamp: Timestamp::new(0, 35),
            },
            payload: Some(Payload {
                hash: PayloadHash([0; 32]),
                payload_round: 0,
                pre_cer: None,
                cer: None,
                operations: vec![],
            }),
        };
        let now = Timestamp::new(0, 35);
        machine.handle(&config, Event::Proposal(Box::new(block.clone()), now));

        let mut block_id = BlockId {
            level: 5,
            round: 0,
            payload_hash: PayloadHash([0; 32]),
        };
        let validator = Validator {
            id: 0,
            power: 256,
            operation: 0,
        };
        let now = Timestamp::new(0, 36);
        machine.handle(
            &config,
            Event::PreVoted(block_id.clone(), validator.clone(), now),
        );
        machine.handle(
            &config,
            Event::Voted(block_id.clone(), validator.clone(), now),
        );

        block.pred_hash = block.hash;
        block.hash = BlockHash([2; 32]);
        block.level = 6;
        block.time_header.timestamp = Timestamp::new(0, 50);
        let now = Timestamp::new(0, 50);
        machine.handle(&config, Event::Proposal(Box::new(block.clone()), now));
        let now = Timestamp::new(0, 51);
        block_id.level += 1;
        machine.handle(
            &config,
            Event::PreVoted(block_id.clone(), validator.clone(), now),
        );
        machine.handle(
            &config,
            Event::Voted(block_id.clone(), validator.clone(), now),
        );

        block.hash = BlockHash([3; 32]);
        block.level = 6;
        block.time_header.round = 1;
        block.time_header.timestamp = Timestamp::new(1, 5);
        let now = Timestamp::new(1, 5);
        machine.handle(&config, Event::Proposal(Box::new(block.clone()), now));
        let now = Timestamp::new(1, 6);
        block_id.round += 2;
        machine.handle(
            &config,
            Event::PreVoted(block_id.clone(), validator.clone(), now),
        );
        machine.handle(
            &config,
            Event::Voted(block_id.clone(), validator.clone(), now),
        );

        block.pred_hash = BlockHash([2; 32]);
        block.hash = BlockHash([4; 32]);
        block.level = 7;
        block.time_header.round = 1;
        block.time_header.timestamp = Timestamp::new(1, 20);
        let now = Timestamp::new(1, 19);
        let (actions, _) = machine.handle(&config, Event::Proposal(Box::new(block.clone()), now));
        assert!(actions
            .iter()
            .find(|a| matches!(a, Action::PreVote { .. }))
            .is_some());
    }
}
