// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cmp::Ordering, collections::BTreeMap, mem, time::Duration};

use serde::{Deserialize, Serialize};

use super::{
    hash::{BlockHash, BlockPayloadHash, NoValue},
    timestamp::{Config, TimeHeader, Timeout, Timestamp, Timing},
    types::{
        Action, Block, BlockId, Certificate, Event, OperationSimple, Payload, PreCertificate, Vote,
        Votes,
    },
    ProposerMap,
};

#[derive(Default, Serialize, Deserialize)]
pub struct Machine {
    inner: Option<Result<Initialized, Transition>>,
}

// We are only interested in proposals in this state
// if the proposal of the same level, add its time header to the collection
// if the proposal of next level, go to next state
#[derive(Serialize, Deserialize)]
struct Transition {
    level: i32,
    hash: BlockHash,
    time_headers: BTreeMap<BlockHash, TimeHeader<false>>,
    timeout_next_level: Option<Timeout>,
}

#[derive(Serialize, Deserialize)]
struct Initialized {
    level: i32,
    // time headers of all possible predecessors
    pred_time_headers: BTreeMap<BlockHash, TimeHeader<true>>,
    this_time_headers: BTreeMap<BlockHash, TimeHeader<false>>,
    hash: BlockHash,
    // current
    this_time_header: TimeHeader<false>,
    pred_hash: BlockHash,
    // block payload
    payload_hash: BlockPayloadHash,
    cer: Option<Certificate>,
    payload_round: i32,
    operations: Vec<OperationSimple>,
    // new block payload
    new_operations: Vec<OperationSimple>,
    locked: Option<(i32, BlockPayloadHash)>,
    prequorum_incomplete: Votes,
    prequorum: PreVotesState,
    timeout_this_level: Option<Timeout>,
    quorum: VotesState,
    timeout_next_level: Option<Timeout>,
    // ahead
    ahead_preendorsements: Vec<Vote>,
    ahead_endorsements: Vec<Vote>,
}

#[derive(Serialize, Deserialize)]
enum PreVotesState {
    Collecting,
    Done {
        // **invariant**
        // `pred_time_headers` should contain corresponding time header
        pred_hash: BlockHash,
        operations: Vec<OperationSimple>,
        pre_cer: PreCertificate,
    },
}

#[derive(Serialize, Deserialize)]
enum VotesState {
    Collecting {
        incomplete: Votes,
    },
    Done {
        round: i32,
        hash: BlockHash,
        payload_hash: BlockPayloadHash,
        cer: Certificate,
    },
}

impl Machine {
    pub fn handle<T, P>(&mut self, config: &Config<T, P>, event: Event, actions: &mut Vec<Action>)
    where
        T: Timing,
        P: ProposerMap,
    {
        match event {
            Event::Proposal(block, now) => {
                actions.push(Action::Proposal {
                    level: block.level,
                    round: block.time_header.round,
                    timestamp: block.time_header.timestamp,
                });
                let new = match self.inner.take() {
                    None => {
                        if block.payload.is_none() {
                            Err(Transition::next_level(config, block, now, actions))
                        } else {
                            let time_header = Self::derive_pred_time_header(
                                &config.timing,
                                0,
                                &block.time_header,
                            );
                            let p = (block.pred_hash.clone(), time_header.clone());
                            Initialized::next_level(
                                Some(p).into_iter().collect(),
                                vec![],
                                vec![],
                                config,
                                block,
                                now,
                                actions,
                            )
                        }
                    }
                    Some(Err(transition)) => {
                        if block.level == transition.level + 1 {
                            let time_headers = transition
                                .time_headers
                                .into_iter()
                                .map(|(k, v)| (k, TimeHeader::into_prev(v)))
                                .collect();
                            Initialized::next_level(
                                time_headers,
                                vec![],
                                vec![],
                                config,
                                block,
                                now,
                                actions,
                            )
                        } else if block.level == transition.level {
                            Err(transition.next_round(block))
                        } else {
                            actions.push(Action::UnexpectedLevel {
                                current: transition.level,
                            });
                            Err(Transition::next_level(config, block, now, actions))
                        }
                    }
                    Some(Ok(initialized)) => {
                        if block.level == initialized.level + 1 {
                            let mut time_headers = initialized.this_time_headers;
                            time_headers
                                .insert(initialized.hash.clone(), initialized.this_time_header);
                            let time_headers = time_headers
                                .into_iter()
                                .map(|(k, v)| (k, TimeHeader::into_prev(v)))
                                .collect();
                            Initialized::next_level(
                                time_headers,
                                initialized.ahead_preendorsements,
                                initialized.ahead_endorsements,
                                config,
                                block,
                                now,
                                actions,
                            )
                        } else if block.level == initialized.level - 1 {
                            let mut initialized = initialized;
                            initialized
                                .pred_time_headers
                                .insert(block.hash, block.time_header.into_prev());
                            Ok(initialized)
                        } else if block.level == initialized.level {
                            let mut initialized = initialized;
                            if !initialized.pred_time_headers.contains_key(&block.pred_hash) {
                                match &block.payload {
                                    // if payload has either certificate or pre-certificate
                                    // we know that the predecessor must exist
                                    Some(Payload {
                                        pre_cer: Some(_), ..
                                    })
                                    | Some(Payload { cer: Some(_), .. }) => {
                                        let time_header = Self::derive_pred_time_header(
                                            &config.timing,
                                            0,
                                            &block.time_header,
                                        );
                                        initialized
                                            .pred_time_headers
                                            .insert(block.pred_hash.clone(), time_header);
                                    }
                                    _ => (),
                                }
                            }
                            initialized.next_round(config, block, now, actions)
                        } else {
                            actions.push(Action::UnexpectedLevel {
                                current: initialized.level,
                            });
                            // block from far future or from the past, go to empty state
                            Err(Transition::next_level(config, block, now, actions))
                        }
                    }
                };
                self.inner = Some(new);
            }
            Event::Preendorsed { vote } => match self.inner.take() {
                Some(Ok(m)) => {
                    self.inner = Some(Ok(m.preendorsed(config, vote, actions)));
                }
                _ => (),
            },
            Event::Endorsed { vote, now } => match self.inner.take() {
                Some(Ok(m)) => {
                    self.inner = Some(Ok(m.endorsed(config, vote, now, actions)));
                }
                _ => (),
            },
            Event::Operation(op) => match &mut self.inner {
                Some(Ok(m)) => m.new_operations.push(op),
                _ => (),
            },
            Event::Timeout => match &mut self.inner {
                None => (),
                Some(Ok(m)) => m.timeout(actions),
                Some(Err(m)) => m.timeout(actions),
            },
        }
    }

    // It is possible we saw no predecessor at all, but we see a prequorum
    // for the predecessor in the current block.
    fn derive_pred_time_header<T>(
        timing: &T,
        pred_round: i32,
        this_th: &TimeHeader<false>,
    ) -> TimeHeader<true>
    where
        T: Timing,
    {
        TimeHeader {
            round: pred_round,
            timestamp: this_th.timestamp
                - timing.offset(this_th.round)
                - timing.round_duration(pred_round),
        }
    }

    pub fn elected_block(&self) -> Option<BlockHash> {
        let initialized = self.inner.as_ref()?.as_ref().ok()?;
        match &initialized.quorum {
            VotesState::Done { hash, .. } => Some(hash.clone()),
            VotesState::Collecting { .. } => None,
        }
    }

    /// current level
    pub fn level(&self) -> Option<i32> {
        let inner = self.inner.as_ref()?;
        match inner {
            Ok(t) => Some(t.level),
            Err(t) => Some(t.level),
        }
    }

    /// current round
    pub fn round(&self) -> Option<i32> {
        let initialized = self.inner.as_ref()?.as_ref().ok()?;
        Some(initialized.this_time_header.round)
    }

    /// timestamp of last proposal
    pub fn timestamp(&self) -> Option<Timestamp> {
        let initialized = self.inner.as_ref()?.as_ref().ok()?;
        Some(initialized.this_time_header.timestamp)
    }

    /// predecessor hash of last proposal
    pub fn predecessor_hash(&self) -> Option<BlockHash> {
        let initialized = self.inner.as_ref()?.as_ref().ok()?;
        Some(initialized.pred_hash.clone())
    }

    /// hash of last proposal
    pub fn hash(&self) -> Option<BlockHash> {
        let initialized = self.inner.as_ref()?.as_ref().ok()?;
        Some(initialized.hash.clone())
    }

    /// payload hash of last proposal
    pub fn payload_hash(&self) -> Option<BlockPayloadHash> {
        let initialized = self.inner.as_ref()?.as_ref().ok()?;
        Some(initialized.payload_hash.clone())
    }
}

impl Transition {
    pub fn next_level<T, P>(
        config: &Config<T, P>,
        block: Block,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) -> Self
    where
        T: Timing,
        P: ProposerMap,
    {
        // bake only if it is transition block (payload is none)
        let timeout_next_level = if block.payload.is_none() {
            block
                .time_header
                .calculate(actions, config, now, block.level)
        } else {
            None
        };
        let timeout_timestamp = timeout_next_level.as_ref().map(|t| t.timestamp);
        actions.extend(timeout_timestamp.map(Action::ScheduleTimeout));
        Transition {
            hash: block.hash.clone(),
            level: block.level,
            time_headers: Some((block.hash, block.time_header)).into_iter().collect(),
            timeout_next_level,
        }
    }

    fn next_round(mut self, block: Block) -> Self {
        self.time_headers.insert(block.hash, block.time_header);
        self
    }
}

impl Initialized {
    pub fn next_level<T, P>(
        pred_time_headers: BTreeMap<BlockHash, TimeHeader<true>>,
        ahead_preendorsements: Vec<Vote>,
        ahead_endorsements: Vec<Vote>,
        config: &Config<T, P>,
        block: Block,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) -> Result<Self, Transition>
    where
        T: Timing,
        P: ProposerMap,
    {
        let pred_time_header = match pred_time_headers.get(&block.pred_hash) {
            Some(v) => v,
            None => {
                actions.push(Action::NoPredecessor);
                return Err(Transition::next_level(config, block, now, actions));
            }
        };
        actions.push(Action::Predecessor {
            round: pred_time_header.round,
            timestamp: pred_time_header.timestamp,
        });
        let payload = if let Some(v) = block.payload {
            v
        } else {
            actions.push(Action::TwoTransitionsInRow);
            return Err(Transition::next_level(config, block, now, actions));
        };

        let current_round = pred_time_header.round_local_coord(&config.timing, now);

        // block from future, let's accept it and write a warning
        let current_round = if current_round < block.time_header.round {
            actions.push(Action::UnexpectedRoundFromFuture {
                current: current_round,
                block_round: block.time_header.round,
            });
            block.time_header.round
        } else {
            current_round
        };

        if current_round == block.time_header.round {
            actions.push(Action::Preendorse(
                block.pred_hash.clone(),
                BlockId {
                    level: block.level,
                    round: current_round,
                    block_payload_hash: payload.hash.clone(),
                },
            ))
        }

        let prequorum = if let Some(pre_cer) = payload.pre_cer {
            actions.push(Action::HavePreCertificate {
                payload_round: pre_cer.payload_round,
            });
            PreVotesState::Done {
                pred_hash: block.pred_hash.clone(),
                operations: payload.operations.clone(),
                pre_cer,
            }
        } else {
            PreVotesState::Collecting
        };
        let timeout_this_level = pred_time_header.calculate(actions, config, now, block.level);
        let delay = Duration::from_millis(200);
        let timeout_timestamp = timeout_this_level.as_ref().map(|t| t.timestamp + delay);
        actions.extend(timeout_timestamp.map(Action::ScheduleTimeout));

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
            prequorum_incomplete: Votes::default(),
            prequorum,
            timeout_this_level,
            quorum: VotesState::Collecting {
                incomplete: Votes::default(),
            },
            timeout_next_level: None,
            ahead_preendorsements,
            ahead_endorsements,
        }
        .reapply_ahead_ops(config, now, actions))
    }

    pub fn next_round<T, P>(
        self,
        config: &Config<T, P>,
        block: Block,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) -> Result<Self, Transition>
    where
        T: Timing,
        P: ProposerMap,
    {
        let mut block = block;
        let payload = if let Some(v) = block.payload.take() {
            v
        } else {
            actions.push(Action::TwoTransitionsInRow);
            return Err(Transition::next_level(config, block, now, actions));
        };
        let block = block;

        if block.pred_hash != self.pred_hash {
            // decide wether should switch or not
            let switch = match (&self.prequorum, &payload.pre_cer) {
                (PreVotesState::Collecting, _) => true,
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
                actions.push(Action::NoSwitchBranch);
                return Ok(self);
            }

            actions.push(Action::SwitchBranch);
        }

        let pred_time_header = match self.pred_time_headers.get(&block.pred_hash) {
            Some(v) => v.clone(),
            None => {
                actions.push(Action::NoPredecessor);
                return Err(Transition::next_level(config, block, now, actions));
            }
        };
        actions.push(Action::Predecessor {
            round: pred_time_header.round,
            timestamp: pred_time_header.timestamp,
        });

        let current_round = pred_time_header.round_local_coord(&config.timing, now);

        // block from future, let's accept it and write a warning
        let current_round = if current_round < block.time_header.round {
            actions.push(Action::UnexpectedRoundFromFuture {
                current: current_round,
                block_round: block.time_header.round,
            });
            block.time_header.round
        } else {
            current_round
        };

        let new_round = block.time_header.round;
        let accept_not_pre_vote =
            self.this_time_header.round < new_round && new_round < current_round;
        let accept_and_pre_vote = new_round == current_round
            && (self.this_time_header.round != new_round || payload.hash == self.payload_hash);
        if accept_not_pre_vote || accept_and_pre_vote {
            let mut self_ = self;
            self_.pred_hash = block.pred_hash;
            let hdr = mem::replace(&mut self_.this_time_header, block.time_header.clone());
            let hash = mem::replace(&mut self_.hash, block.hash.clone());
            self_.this_time_headers.insert(hash, hdr);
            self_.payload_hash = payload.hash;
            self_.payload_round = payload.payload_round;
            self_.operations = payload.operations;
            self_.cer = payload.cer;
            // self_.new_operations.clear();

            let new_payload_round = payload.pre_cer.as_ref().map(|new| new.payload_round);

            self_.prequorum_incomplete = Votes::default();
            match (&self_.prequorum, payload.pre_cer) {
                (PreVotesState::Done { ref pre_cer, .. }, Some(new))
                    if new.payload_round > pre_cer.payload_round =>
                {
                    actions.push(Action::HavePreCertificate {
                        payload_round: new.payload_round,
                    });
                    self_.prequorum = PreVotesState::Done {
                        pred_hash: self_.pred_hash.clone(),
                        operations: self_.operations.clone(),
                        pre_cer: new,
                    };
                }
                (PreVotesState::Done { .. }, _) => (),
                (PreVotesState::Collecting, Some(new)) => {
                    actions.push(Action::HavePreCertificate {
                        payload_round: new.payload_round,
                    });
                    self_.prequorum = PreVotesState::Done {
                        pred_hash: self_.pred_hash.clone(),
                        operations: self_.operations.clone(),
                        pre_cer: new,
                    };
                }
                _ => (),
            };
            // TODO: rewrite collecting, this block introduce a bug,
            // need to collect few rounds in parallel
            if let VotesState::Collecting { ref mut incomplete } = &mut self_.quorum {
                *incomplete = Votes::default();
            }

            self_.timeout_next_level = match &self_.quorum {
                VotesState::Done { ref hash, .. } => self_
                    .this_time_headers
                    .get(hash)
                    .expect("invariant")
                    .calculate(actions, config, now, self_.level),
                VotesState::Collecting { .. } => None,
            };
            self_.timeout_this_level =
                pred_time_header.calculate(actions, config, now, block.level);

            let delay = Duration::from_millis(200);
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
            actions.extend(t.map(Action::ScheduleTimeout));
            // TODO: `locked_no_preendorse` mitten test sometimes failing
            let will_pre_vote = accept_and_pre_vote
                && match &self_.locked {
                    Some((ref locked_round, ref locked_hash)) => {
                        new_payload_round.unwrap_or(-1) >= *locked_round
                            || locked_hash.eq(&self_.payload_hash)
                    }
                    None => true,
                };
            let block_id = BlockId {
                level: block.level,
                round: current_round,
                block_payload_hash: self_.payload_hash.clone(),
            };
            if will_pre_vote {
                actions.push(Action::Preendorse(
                    self_.pred_hash.clone(),
                    block_id.clone(),
                ))
            }
            if let PreVotesState::Done { .. } = &self_.prequorum {
                actions.push(Action::Endorse(self_.pred_hash.clone(), block_id))
            }

            Ok(self_.reapply_ahead_ops(config, now, actions))
        } else {
            actions.push(Action::UnexpectedRoundBounded {
                last: self.this_time_header.round,
                current: current_round,
            });
            Ok(self)
        }
    }

    fn reapply_ahead_ops<T, P>(
        mut self,
        config: &Config<T, P>,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) -> Self
    where
        T: Timing,
        P: ProposerMap,
    {
        for vote in mem::take(&mut self.ahead_preendorsements) {
            self = self.preendorsed(config, vote, actions);
        }

        for vote in mem::take(&mut self.ahead_endorsements) {
            self = self.endorsed(config, vote, now, actions);
        }

        self
    }

    fn preendorsed<T, P>(
        mut self,
        config: &Config<T, P>,
        vote: Vote,
        actions: &mut Vec<Action>,
    ) -> Self
    where
        T: Timing,
        P: ProposerMap,
    {
        let block_id = &vote.op.contents;
        if block_id.level != self.level
            || block_id.block_payload_hash != self.payload_hash.clone()
            || block_id.round != self.this_time_header.round
        {
            return self;
        }

        let block_id = vote.op.contents.clone();
        let branch = vote.op.branch.clone();
        self.prequorum_incomplete += vote;

        if self.prequorum_incomplete.power >= config.quorum {
            if let Some((ref round, ref payload_hash)) = &self.locked {
                if block_id.round.eq(round) && block_id.block_payload_hash.eq(payload_hash) {
                    return self;
                }
            }

            actions.push(Action::HavePreCertificate {
                payload_round: block_id.round,
            });
            self.locked = Some((block_id.round, block_id.block_payload_hash.clone()));
            self.prequorum = PreVotesState::Done {
                pred_hash: self.pred_hash.clone(),
                operations: self.operations.clone(),
                pre_cer: PreCertificate {
                    payload_hash: block_id.block_payload_hash.clone(),
                    payload_round: block_id.round,
                    votes: mem::take(&mut self.prequorum_incomplete),
                },
            };
            actions.push(Action::Endorse(branch, block_id));
        }

        self
    }

    fn endorsed<T, P>(
        mut self,
        config: &Config<T, P>,
        vote: Vote,
        now: Timestamp,
        actions: &mut Vec<Action>,
    ) -> Self
    where
        T: Timing,
        P: ProposerMap,
    {
        let block_id = vote.op.contents.clone();

        if let VotesState::Done {
            ref round,
            ref payload_hash,
            ref mut cer,
            ..
        } = &mut self.quorum
        {
            if round.eq(&block_id.round) && payload_hash.eq(&block_id.block_payload_hash) {
                cer.votes += vote;
            }
            return self;
        }

        if block_id.level != self.level
            || block_id.block_payload_hash != self.payload_hash.clone()
            || block_id.round != self.this_time_header.round
        // let's still accept late endorsements until we receive new proposal
        // despite our `current_round` is bigger
        // || block_id.round < current_round
        {
            if block_id.level > self.level
                || (block_id.level == self.level && block_id.round > self.this_time_header.round)
            {
                self.ahead_endorsements.push(vote);
            }
            return self;
        }

        let votes = match &mut self.quorum {
            VotesState::Collecting { ref mut incomplete } => incomplete,
            VotesState::Done {
                ref mut cer,
                ref hash,
                ..
            } => {
                if self.hash.eq(hash) {
                    cer.votes += vote;
                }
                return self;
            }
        };

        *votes += vote;

        if votes.power >= config.quorum {
            actions.push(Action::HaveCertificate);
            self.quorum = VotesState::Done {
                hash: self.hash.clone(),
                round: block_id.round,
                payload_hash: block_id.block_payload_hash,
                cer: Certificate {
                    votes: mem::take(votes),
                },
            };

            self.timeout_next_level = self
                .this_time_header
                .calculate(actions, config, now, self.level);

            if let Some(ref n) = &self.timeout_next_level {
                let timestamp = n.timestamp;
                match &self.timeout_this_level {
                    Some(ref t) if t.timestamp < timestamp => (),
                    _ => actions.push(Action::ScheduleTimeout(timestamp)),
                }
            }
        }
        self
    }
}

impl Transition {
    fn timeout(&mut self, actions: &mut Vec<Action>) {
        if let Some(Timeout {
            proposer,
            round,
            timestamp,
        }) = self.timeout_next_level.take()
        {
            let new_block = Block {
                pred_hash: self.hash.clone(),
                hash: BlockHash::no_value(),
                level: self.level + 1,
                time_header: TimeHeader { round, timestamp },
                payload: Some(Payload {
                    hash: BlockPayloadHash::no_value(),
                    payload_round: round,
                    pre_cer: None,
                    cer: None,
                    operations: vec![],
                }),
            };
            actions.push(Action::Propose(new_block, proposer, false));
        }
    }
}

impl Initialized {
    fn timeout(&mut self, actions: &mut Vec<Action>) {
        let (
            this,
            Timeout {
                proposer,
                round,
                timestamp,
            },
        ) = match (&self.timeout_this_level, &self.timeout_next_level) {
            (Some(ref this), Some(ref next)) => {
                if this.timestamp < next.timestamp {
                    (true, this.clone())
                } else {
                    (false, next.clone())
                }
            }
            (Some(ref this), None) => (true, this.clone()),
            (None, Some(ref next)) => (false, next.clone()),
            (None, None) => return,
        };
        let time_header = TimeHeader { round, timestamp };

        let new_block = if this {
            self.timeout_this_level = None;
            match &self.prequorum {
                PreVotesState::Done {
                    pred_hash,
                    operations,
                    pre_cer,
                } => Block {
                    pred_hash: pred_hash.clone(),
                    hash: BlockHash::no_value(),
                    level: self.level,
                    time_header,
                    payload: Some(Payload {
                        hash: pre_cer.payload_hash.clone(),
                        payload_round: pre_cer.payload_round,
                        pre_cer: Some(pre_cer.clone()),
                        cer: self.cer.clone(),
                        operations: operations.clone(),
                    }),
                },
                PreVotesState::Collecting => Block {
                    pred_hash: self.pred_hash.clone(),
                    hash: BlockHash::no_value(),
                    level: self.level,
                    time_header,
                    payload: Some(Payload {
                        hash: BlockPayloadHash::no_value(),
                        payload_round: 0,
                        pre_cer: None,
                        cer: self.cer.clone(),
                        operations: self.operations.clone(),
                    }),
                },
            }
        } else {
            self.timeout_next_level = None;
            match &self.quorum {
                VotesState::Done { cer, hash, .. } => Block {
                    pred_hash: hash.clone(),
                    hash: BlockHash::no_value(),
                    level: self.level + 1,
                    time_header,
                    payload: Some(Payload {
                        hash: BlockPayloadHash::no_value(),
                        payload_round: round,
                        pre_cer: None,
                        cer: Some(cer.clone()),
                        operations: self.new_operations.clone(),
                    }),
                },
                _ => return,
            }
        };

        actions.push(Action::Propose(new_block, proposer, this));
        let delay = Duration::from_millis(200);
        let t = match (&self.timeout_this_level, &self.timeout_next_level) {
            (Some(ref this), None) => Some(this.timestamp + delay),
            (None, Some(ref next)) => Some(next.timestamp),
            _ => None,
        };
        actions.extend(t.map(Action::ScheduleTimeout));
    }
}
