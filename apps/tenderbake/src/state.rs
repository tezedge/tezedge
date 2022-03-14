// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{cmp::Ordering, mem, fmt};
use alloc::boxed::Box;

use arrayvec::ArrayVec;

use super::{
    timestamp::Timestamp,
    validator::ValidatorMap,
    block::{BlockId, Votes, Prequorum, Quorum, Payload, BlockInfo},
    interface::{Config, Proposal, Preendorsement, Endorsement, Event, Action},
};

pub struct LockedRound {
    round: i32,
    payload_hash: [u8; 32],
}

pub struct EndorsablePayload<P>
where
    P: Payload,
{
    pred: BlockInfo<P>,
    prequorum: Prequorum<P::Item>,
    locked: Option<LockedRound>,
}

pub struct ElectedBlock<P>
where
    P: Payload,
{
    head: BlockInfo<P>,
    quorum: Quorum<P::Item>,
}

pub enum ClosestRound {
    Never,
    ThisLevel { round: i32, timestamp: Timestamp },
    NextLevel { round: i32, timestamp: Timestamp },
}

impl fmt::Display for ClosestRound {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClosestRound::Never => write!(f, "nor this level not next level"),
            ClosestRound::ThisLevel { round, timestamp } => {
                write!(f, "this level, round: {round}, {timestamp}")
            }
            ClosestRound::NextLevel { round, timestamp } => {
                write!(f, "next level, round: {round}, {timestamp}")
            }
        }
    }
}

impl Default for ClosestRound {
    fn default() -> Self {
        ClosestRound::Never
    }
}

pub struct Machine<P>
where
    P: Payload,
{
    proposal: Proposal<P>,
    incomplete_prequorum: Votes<P::Item>,
    incomplete_quorum: Votes<P::Item>,

    endorsable_payload: Option<EndorsablePayload<P>>,
    elected_block: Option<ElectedBlock<P>>,
    round_state: i32,
    payload: P,

    closest_timestamp_this_level: Option<Timestamp>,
    closest_timestamp_next_level: Option<Timestamp>,
    closest_round: ClosestRound,
}

impl<P> Machine<P>
where
    P: Payload + Clone,
    P::Item: Clone,
{
    pub fn empty() -> Self {
        Machine {
            proposal: Proposal {
                pred: BlockInfo::GENESIS,
                head: BlockInfo::GENESIS,
            },
            incomplete_prequorum: Votes::default(),
            incomplete_quorum: Votes::default(),
            endorsable_payload: None,
            elected_block: None,
            round_state: 0,
            payload: P::EMPTY,
            closest_timestamp_this_level: None,
            closest_timestamp_next_level: None,
            closest_round: ClosestRound::Never,
        }
    }

    /// Event cause actions:
    /// Proposal -> [ ScheduleTimeout? Preendorse? ]
    /// Preendorsement -> [ ScheduleTimeout? Endorse? ]
    /// Endorsement -> [ ScheduleTimeout? ]
    /// Timeout -> [ Propose ]
    /// PayloadItem -> [ ]
    pub fn handle<V>(&mut self, config: &Config, map: &V, event: Event<P>) -> ArrayVec<Action<P>, 2>
    where
        V: ValidatorMap,
    {
        match event {
            Event::Proposal(proposal, now) => self.proposal(config, map, *proposal, now),
            Event::Preendorsement(content, op) => self.preendorsement(config, map, content, op),
            Event::Endorsement(content, op, now) => self.endorsement(config, map, now, content, op),
            Event::Timeout => self.timeout(),
            Event::PayloadItem(item) => {
                self.payload.update(item);
                ArrayVec::default()
            }
        }
    }

    fn timeout(&mut self) -> ArrayVec<Action<P>, 2> {
        let level = self.proposal.head.block_id.level;
        log::info!(" .  will bake level: {level}, {}", self.closest_round);
        let proposal = match mem::take(&mut self.closest_round) {
            ClosestRound::Never => None,
            ClosestRound::NextLevel { round, timestamp } => {
                let elected_block = self.elected_block.as_ref().expect("msg");
                let new_proposal = Proposal {
                    pred: elected_block.head.clone(),
                    head: BlockInfo {
                        hash: [0; 32],
                        block_id: BlockId {
                            level: self.proposal.head.block_id.level + 1,
                            round,
                            payload_hash: [0; 32],
                            payload_round: round,
                        },
                        timestamp,
                        transition: false,
                        prequorum: None,
                        quorum: Some(elected_block.quorum.clone()),
                        payload: mem::replace(&mut self.payload, P::EMPTY),
                    },
                };
                self.closest_timestamp_next_level = None;
                Some(Box::new(new_proposal))
            }
            ClosestRound::ThisLevel { round, timestamp } => {
                let mut pred = self.proposal.pred.clone();
                let mut head = self.proposal.head.clone();
                head.timestamp = timestamp;
                head.block_id.round = round;
                if let Some(ref endorsable_payload) = &self.endorsable_payload {
                    head.block_id.payload_hash = endorsable_payload.prequorum.block_id.payload_hash;
                    head.block_id.payload_round =
                        endorsable_payload.prequorum.block_id.payload_round;
                    head.prequorum = Some(endorsable_payload.prequorum.clone());
                    pred = endorsable_payload.pred.clone();
                } else {
                    head.block_id.payload_hash = [0; 32];
                    head.block_id.payload_round = round;
                    head.prequorum = None;
                }
                self.closest_timestamp_this_level = None;
                Some(Box::new(Proposal { pred, head }))
            }
        };
        proposal.map(Action::Propose).into_iter().collect()
    }

    fn proposal<V>(
        &mut self,
        config: &Config,
        map: &V,
        new_proposal: Proposal<P>,
        now: Timestamp,
    ) -> ArrayVec<Action<P>, 2>
    where
        V: ValidatorMap,
    {
        log::info!("Proposal: {}", new_proposal.head);

        let mut actions = ArrayVec::new();

        let old_block_id = self.proposal.head.block_id.clone();
        match self
            .proposal
            .head
            .block_id
            .level
            .cmp(&new_proposal.head.block_id.level)
        {
            Ordering::Less => {
                // we received a block for a next level
                // TODO: transition
                let pred_duration = config.round_duration(new_proposal.pred.block_id.round);
                let start_this_level = new_proposal.pred.timestamp + pred_duration;
                log::info!(" .  start this level: {start_this_level}");
                self.round_state = config.round(now, start_this_level);
                log::info!(" .  this round: {}", self.round_state);

                if self.round_state < new_proposal.head.block_id.round {
                    // proposal from future, ignore
                    log::warn!(" .  ignore proposal from future");
                    return ArrayVec::default();
                }

                self.closest_timestamp_this_level = None;
                self.closest_timestamp_next_level = None;
                self.closest_round = ClosestRound::Never;

                self.endorsable_payload = None;
                self.elected_block = None;

                self.update_endorsable_payload(&new_proposal);

                if self.round_state == new_proposal.head.block_id.round {
                    let block_id = new_proposal.head.block_id.clone();
                    actions.extend(Self::preendorse(map, block_id, new_proposal.pred.hash));
                }

                self.proposal = new_proposal;

                // TODO: transition doesn't work
                // no endorsements needed for the first block
                if self.proposal.head.transition {
                    self.elected_block = Some(ElectedBlock {
                        head: self.proposal.head.clone(),
                        quorum: Quorum {
                            votes: Votes::default(),
                        },
                    });
                    self.update_timeout_next_level(config, map, now);
                }
            }
            Ordering::Greater => {
                // The baker is ahead, a reorg may have happened. Do nothing:
                // wait for the node to send us the branch's head. This new head
                // should have a fitness that is greater than our current
                // proposal and thus, its level should be at least the same as
                // our current proposal's level.
                return ArrayVec::default();
            }
            Ordering::Equal => {
                let pred_duration = config.round_duration(new_proposal.pred.block_id.round);
                let start_this_level = new_proposal.pred.timestamp + pred_duration;
                log::info!(" .  start this level: {start_this_level}");
                self.round_state = config.round(now, start_this_level);
                log::info!(" .  this round: {}", self.round_state);

                if self.proposal.pred.hash != new_proposal.pred.hash {
                    let switch = match (&self.endorsable_payload, &self.proposal.head.prequorum) {
                        (None, _) => {
                            // The new branch contains a PQC (and we do not) or a better
                            // fitness, we switch.
                            true
                        }
                        (Some(_), None) => {
                            // We have a better PQC, we don't switch as we are able to
                            // propose a better chain if we stay on our current one.
                            false
                        }
                        (Some(ref current_prequorum), Some(new_prequorum)) => {
                            let r = current_prequorum.prequorum.block_id.round;
                            match r.cmp(&new_prequorum.block_id.round) {
                                // The other's branch PQC is lower than ours, do not switch
                                Ordering::Greater => false,
                                // Their PQC is better than ours: we switch
                                Ordering::Less => true,
                                // There is a PQC on two branches with the same round and
                                // the same level but not the same predecessor : it's
                                // impossible unless if there was some double-baking. This
                                // shouldn't happen but do nothing anyway.
                                Ordering::Equal => false,
                            }
                        }
                    };

                    if !switch {
                        log::warn!(" .  ignore proposal, will not change branch");
                        return ArrayVec::default();
                    }
                }

                let head = &self.proposal.head;
                if new_proposal.head.block_id.round < self.round_state
                    && head.block_id.round < new_proposal.head.block_id.round
                {
                    self.update_endorsable_payload(&new_proposal);
                    self.proposal = new_proposal;
                } else if new_proposal.head.block_id.round == self.round_state
                    && (head.block_id.round != new_proposal.head.block_id.round
                        || head.block_id.payload_hash == new_proposal.head.block_id.payload_hash)
                {
                    self.update_endorsable_payload(&new_proposal);
                    let head = &self.proposal.head;
                    let will_preendorse = match &self.endorsable_payload {
                        Some(EndorsablePayload {
                            locked: Some(ref l),
                            ..
                        }) => {
                            matches!(
                                &head.prequorum,
                                Some(ref p) if p.block_id.round >= l.round,
                            ) || l.payload_hash == head.block_id.payload_hash
                        }
                        _ => true,
                    };
                    if will_preendorse {
                        let block_id = new_proposal.head.block_id.clone();
                        actions.extend(Self::preendorse(map, block_id, new_proposal.pred.hash));
                    }
                    self.proposal = new_proposal;
                } else {
                    log::warn!(" .  ignore proposal");
                    return ArrayVec::default();
                }
            }
        };
        if old_block_id != self.proposal.head.block_id {
            self.incomplete_prequorum = Votes::default();
            self.incomplete_quorum = Votes::default();
            self.payload = P::EMPTY;
        }
        let timeout = self
            .update_timeout_this_level(config, map)
            .map(Action::ScheduleTimeout);
        actions.extend(timeout);

        actions
    }

    fn update_endorsable_payload(&mut self, new_proposal: &Proposal<P>) {
        if let Some(ref new) = &new_proposal.head.prequorum {
            if matches!(&self.endorsable_payload, Some(ref e) if &e.prequorum >= new) {
                // our prequorum is better, do nothing
                return;
            }
            // take locked round if any
            let locked = self
                .endorsable_payload
                .as_mut()
                .and_then(|e| e.locked.take());
            self.endorsable_payload = Some(EndorsablePayload {
                pred: new_proposal.pred.clone(),
                prequorum: new.clone(),
                // put it back
                locked,
            });
        }
    }

    fn preendorse<V>(map: &V, block_id: BlockId, pred_hash: [u8; 32]) -> Option<Action<P>>
    where
        V: ValidatorMap,
    {
        map.preendorser(block_id.level, block_id.round)
            .map(|validator| {
                log::info!(" .  inject preendorsement: {block_id}");
                Action::Preendorse {
                    pred_hash,
                    content: Preendorsement {
                        validator,
                        block_id,
                    },
                }
            })
    }

    fn preendorsement<V>(
        &mut self,
        config: &Config,
        map: &V,
        content: Preendorsement,
        op: P::Item,
    ) -> ArrayVec<Action<P>, 2>
    where
        V: ValidatorMap,
    {
        let head = &self.proposal.head;
        if head.block_id == content.block_id && head.block_id.round == self.round_state {
            self.incomplete_prequorum += (content.validator, op);
        }

        if self.incomplete_prequorum.power < config.consensus_threshold {
            return ArrayVec::default();
        }

        match &self.endorsable_payload {
            Some(EndorsablePayload {
                locked: Some(ref l),
                ..
            }) if l.round == head.block_id.round => return ArrayVec::default(),
            _ => (),
        }

        self.endorsable_payload = Some(EndorsablePayload {
            pred: self.proposal.pred.clone(),
            prequorum: Prequorum {
                block_id: head.block_id.clone(),
                votes: self.incomplete_prequorum.clone(),
            },
            locked: Some(LockedRound {
                round: head.block_id.round,
                payload_hash: head.block_id.payload_hash,
            }),
        });
        log::info!(" .  lock {head}");

        let mut actions = ArrayVec::new();

        let endorsement = Self::endorse(map, head.block_id.clone(), self.proposal.pred.hash);
        let timeout = self
            .update_timeout_this_level(config, map)
            .map(Action::ScheduleTimeout);
        actions.extend(endorsement);
        actions.extend(timeout);

        actions
    }

    fn endorse<V>(map: &V, block_id: BlockId, pred_hash: [u8; 32]) -> Option<Action<P>>
    where
        V: ValidatorMap,
    {
        map.endorser(block_id.level, block_id.round)
            .map(|validator| {
                log::info!(" .  inject endorsement: {block_id}");
                Action::Endorse {
                    pred_hash,
                    content: Endorsement {
                        validator,
                        block_id,
                    },
                }
            })
    }

    fn endorsement<V>(
        &mut self,
        config: &Config,
        map: &V,
        now: Timestamp,
        content: Endorsement,
        op: P::Item,
    ) -> ArrayVec<Action<P>, 2>
    where
        V: ValidatorMap,
    {
        let head = &self.proposal.head;
        if head.block_id == content.block_id && head.block_id.round == self.round_state {
            self.incomplete_quorum += (content.validator, op);
        }

        if self.incomplete_quorum.power >= config.consensus_threshold
            && self.elected_block.is_none()
        {
            let elected = ElectedBlock {
                head: head.clone(),
                quorum: Quorum {
                    votes: self.incomplete_quorum.clone(),
                },
            };
            log::info!(" .  {}", elected.head);
            self.elected_block = Some(elected);

            return self
                .update_timeout_next_level(config, map, now)
                .map(Action::ScheduleTimeout)
                .into_iter()
                .collect();
        }

        ArrayVec::default()
    }

    fn update_timeout_next_level<V>(
        &mut self,
        config: &Config,
        map: &V,
        now: Timestamp,
    ) -> Option<Timestamp>
    where
        V: ValidatorMap,
    {
        if let Some(ref elected_block) = self.elected_block {
            let elected_round = elected_block.head.block_id.round;
            let elected_duration = config.round_duration(elected_round);
            let start_next_level = elected_block.head.timestamp + elected_duration;
            let round = config.round(now, start_next_level);
            if let Some(round) = map.proposer(self.proposal.head.block_id.level + 1, round) {
                let d = config.minimal_block_delay * (round as u32)
                    + config.delay_increment_per_round * (round * (round - 1) / 2) as u32;
                let timestamp = start_next_level + d;
                self.closest_timestamp_next_level = Some(timestamp);
                log::info!(" .  closest round next level: {round}, {timestamp}");
                if let Some(alt) = self.closest_timestamp_this_level {
                    if alt < timestamp {
                        return None;
                    }
                }
                self.closest_round = ClosestRound::NextLevel { round, timestamp };
                return Some(timestamp);
            }
        }

        None
    }

    // call when endorsable payload changed
    fn update_timeout_this_level<V>(&mut self, config: &Config, map: &V) -> Option<Timestamp>
    where
        V: ValidatorMap,
    {
        if let Some(round) = map.proposer(self.proposal.head.block_id.level, self.round_state + 1) {
            let pred = self
                .endorsable_payload
                .as_ref()
                .map(|e| &e.pred)
                .unwrap_or(&self.proposal.pred);
            let endorsable_duration = config.round_duration(pred.block_id.round);
            let start_this_level = pred.timestamp + endorsable_duration;
            let d = config.minimal_block_delay * (round as u32)
                + config.delay_increment_per_round * (round * (round - 1) / 2) as u32;
            let timestamp = start_this_level + d;
            self.closest_timestamp_this_level = Some(timestamp);
            log::info!(" .  closest round this level: {round}, {timestamp}");
            if let Some(alt) = self.closest_timestamp_next_level {
                if alt < timestamp {
                    return None;
                }
            }
            self.closest_round = ClosestRound::ThisLevel { round, timestamp };
            return Some(timestamp);
        }

        None
    }
}
