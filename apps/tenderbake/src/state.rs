// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use core::{cmp::Ordering, mem, fmt};
use alloc::{boxed::Box, collections::BTreeMap};

use arrayvec::ArrayVec;

use super::{
    timestamp::Timestamp,
    validator::ValidatorMap,
    block::{BlockId, Votes, Prequorum, Quorum, Payload, BlockInfo},
    interface::{Config, Pred, Preendorsement, Endorsement, Event, Action},
};

pub struct LockedRound {
    round: i32,
    payload_hash: [u8; 32],
}

pub struct EndorsablePayload<Id, P>
where
    P: Payload,
{
    pred_timestamp: Timestamp,
    pred_round: i32,
    pred_hash: [u8; 32],
    prequorum: Prequorum<Id, P::Item>,
    locked: Option<LockedRound>,
}

pub struct ElectedBlock<Id, P>
where
    P: Payload,
{
    head: BlockInfo<Id, P>,
    quorum: Quorum<Id, P::Item>,
}

pub enum ClosestRound<Id> {
    Never,
    ThisLevel { proposer: Id, round: i32, timestamp: Timestamp },
    NextLevel { proposer: Id, round: i32, timestamp: Timestamp },
}

impl<Id> fmt::Display for ClosestRound<Id>
where
    Id: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClosestRound::Never => write!(f, "nor this level not next level"),
            ClosestRound::ThisLevel { proposer, round, timestamp } => {
                write!(f, "this level, round: {round}, {timestamp}, proposer: {proposer}")
            }
            ClosestRound::NextLevel { proposer, round, timestamp } => {
                write!(f, "next level, round: {round}, {timestamp}, proposer: {proposer}")
            }
        }
    }
}

impl<Id> Default for ClosestRound<Id> {
    fn default() -> Self {
        ClosestRound::Never
    }
}

pub struct Machine<Id, P>
where
    P: Payload,
{
    cache: BTreeMap<[u8; 32], Pred>,

    pred: Pred,
    head: BlockInfo<Id, P>,
    incomplete_prequorum: Votes<Id, P::Item>,
    incomplete_quorum: Votes<Id, P::Item>,

    endorsable_payload: Option<EndorsablePayload<Id, P>>,
    elected_block: Option<ElectedBlock<Id, P>>,
    payload: P,

    closest_timestamp_this_level: Option<Timestamp>,
    closest_timestamp_next_level: Option<Timestamp>,
    closest_round: ClosestRound<Id>,
}

impl<Id, P> Machine<Id, P>
where
    P: Payload + Clone,
    P::Item: Clone,
    Id: Clone + Ord + fmt::Display,
{
    pub fn empty() -> Self {
        Machine {
            cache: BTreeMap::new(),
            pred: Pred {
                timestamp: Timestamp {
                    unix_epoch: Default::default(),
                },
                level: 0,
                round: 0,
                transition: true,
            },
            head: BlockInfo::GENESIS,
            incomplete_prequorum: Votes::default(),
            incomplete_quorum: Votes::default(),
            endorsable_payload: None,
            elected_block: None,
            payload: P::EMPTY,
            closest_timestamp_this_level: None,
            closest_timestamp_next_level: None,
            closest_round: ClosestRound::Never,
        }
    }

    pub fn is_proposal_acceptable(&self, pred_hash: &[u8; 32]) -> bool {
        self.cache.contains_key(pred_hash)
    }

    /// Event cause actions:
    /// Proposal -> [ ScheduleTimeout? Preendorse? ]
    /// Preendorsement -> [ ScheduleTimeout? Endorse? ]
    /// Endorsement -> [ ScheduleTimeout? ]
    /// Timeout -> [ Propose ]
    /// PayloadItem -> [ ]
    pub fn handle<V>(
        &mut self,
        config: &Config,
        map: &V,
        event: Event<Id, P>,
    ) -> ArrayVec<Action<Id, P>, 2>
    where
        V: ValidatorMap<Id = Id>,
    {
        match event {
            Event::InitialProposal(hash, pred, now) => {
                log::info!("Proposal: {pred}");
                self.cache.insert(hash, pred.clone());
                if pred.transition {
                    let elected_duration = config.round_duration(pred.round);
                    let start_next_level = pred.timestamp + elected_duration;
                    let round = config.round(now, start_next_level);
                    if let Some((round, proposer)) = map.proposer(pred.level + 1, round) {
                        let mut head = BlockInfo::GENESIS;
                        head.hash = hash;
                        head.block_id.level = pred.level;
                        self.elected_block = Some(ElectedBlock {
                            head,
                            quorum: Quorum {
                                votes: Votes::default(),
                            },
                        });
                        let d = config.minimal_block_delay * (round as u32)
                            + config.delay_increment_per_round * (round * (round - 1) / 2) as u32;
                        let timestamp = start_next_level + d;
                        self.closest_timestamp_next_level = Some(timestamp);
                        self.closest_round = ClosestRound::NextLevel { proposer, round, timestamp };
                        Some(Action::ScheduleTimeout(timestamp)).into_iter().collect()
                    } else {
                        ArrayVec::default()
                    }
                } else {
                    ArrayVec::default()
                }
            }
            Event::Proposal(proposal, now) => self.proposal(config, map, *proposal, now),
            Event::Preendorsement(content, op, now) => {
                self.preendorsement(config, map, now, content, op)
            }
            Event::Endorsement(content, op, now) => self.endorsement(config, map, now, content, op),
            Event::Timeout => self.timeout(),
            Event::PayloadItem(item) => {
                self.payload.update(item);
                ArrayVec::default()
            }
        }
    }

    fn timeout(&mut self) -> ArrayVec<Action<Id, P>, 2> {
        let level = self.head.block_id.level;
        log::info!(" .  will bake level: {level}, {}", self.closest_round);
        let block = match mem::take(&mut self.closest_round) {
            ClosestRound::Never => None,
            ClosestRound::NextLevel { proposer, round, timestamp } => {
                let elected_block = self.elected_block.as_ref().expect("msg");
                let new_block = BlockInfo {
                    pred_hash: elected_block.head.hash,
                    hash: [0; 32],
                    block_id: BlockId {
                        level: elected_block.head.block_id.level + 1,
                        round,
                        payload_hash: [0; 32],
                        payload_round: round,
                    },
                    timestamp,
                    transition: false,
                    prequorum: None,
                    quorum: Some(elected_block.quorum.clone()),
                    payload: mem::replace(&mut self.payload, P::EMPTY),
                };
                self.closest_timestamp_next_level = None;
                Some((Box::new(new_block), proposer))
            }
            ClosestRound::ThisLevel { proposer, round, timestamp } => {
                let mut head = self.head.clone();
                head.timestamp = timestamp;
                head.block_id.round = round;
                if let Some(ref endorsable_payload) = &self.endorsable_payload {
                    head.block_id.payload_hash = endorsable_payload.prequorum.block_id.payload_hash;
                    head.block_id.payload_round =
                        endorsable_payload.prequorum.block_id.payload_round;
                    head.prequorum = Some(endorsable_payload.prequorum.clone());
                    head.pred_hash = endorsable_payload.pred_hash;
                } else {
                    head.block_id.payload_hash = [0; 32];
                    head.block_id.payload_round = round;
                    head.prequorum = None;
                }
                self.closest_timestamp_this_level = None;
                Some((Box::new(head), proposer))
            }
        };
        block.map(|(head, proposer)| Action::Propose(head, proposer)).into_iter().collect()
    }

    fn proposal<V>(
        &mut self,
        config: &Config,
        map: &V,
        new_proposal: BlockInfo<Id, P>,
        now: Timestamp,
    ) -> ArrayVec<Action<Id, P>, 2>
    where
        V: ValidatorMap<Id = Id>,
    {
        log::info!("Proposal: {new_proposal}");

        let mut actions = ArrayVec::new();

        match self
            .head
            .block_id
            .level
            .cmp(&new_proposal.block_id.level)
        {
            Ordering::Less => {
                // we received a block for a next level
                // TODO: transition

                self.closest_timestamp_this_level = None;
                self.closest_timestamp_next_level = None;
                self.closest_round = ClosestRound::Never;

                self.endorsable_payload = None;
                self.elected_block = None;

                let pred = self
                    .cache
                    .get(&new_proposal.pred_hash)
                    .expect("the proposal cannot be accepted, use `is_proposal_acceptable`");

                let current_round = pred.round_local_coord(config, now);
                log::info!(" .  this round: {current_round}");

                if current_round < new_proposal.block_id.round {
                    // proposal from future, ignore
                    log::warn!(" .  ignore proposal from future");
                    return ArrayVec::default();
                }

                self.pred = pred.clone();
                self.update_endorsable_payload(&new_proposal);

                if current_round == new_proposal.block_id.round {
                    let block_id = new_proposal.block_id.clone();
                    actions.extend(Self::preendorse(block_id, new_proposal.pred_hash));
                }

                self.head = new_proposal;

                // TODO: transition doesn't work
                // no endorsements needed for the first block
                // if self.head.transition {
                //     self.elected_block = Some(ElectedBlock {
                //         head: self.head.clone(),
                //         quorum: Quorum {
                //             votes: Votes::default(),
                //         },
                //     });
                //     self.update_timeout_next_level(config, map, now);
                // }
            }
            Ordering::Greater => {
                log::info!(" .  branch switch may have happened");
                // The baker is ahead, a reorg may have happened. Do nothing:
                // wait for the node to send us the branch's head. This new head
                // should have a fitness that is greater than our current
                // proposal and thus, its level should be at least the same as
                // our current proposal's level.
                return ArrayVec::default();
            }
            Ordering::Equal => {
                if self.head.pred_hash != new_proposal.pred_hash {
                    let switch = match (&self.endorsable_payload, &self.head.prequorum) {
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
                    let old_pred = self.pred.clone();
                    self.pred = self
                        .cache
                        .get(&new_proposal.pred_hash)
                        .expect("the proposal cannot be accepted, use `is_proposal_acceptable`")
                        .clone();
                    log::info!(" .  branch switch {old_pred} -> {}", self.pred);
                }

                let current_round = self.pred.round_local_coord(config, now);
                log::info!(" .  this round: {current_round}");

                let head = &self.head;
                if new_proposal.block_id.round < current_round
                    && head.block_id.round < new_proposal.block_id.round
                {
                    self.update_endorsable_payload(&new_proposal);
                    self.head = new_proposal;
                } else if new_proposal.block_id.round == current_round
                    && (head.block_id.round != new_proposal.block_id.round
                        || head.block_id.payload_hash == new_proposal.block_id.payload_hash)
                {
                    self.update_endorsable_payload(&new_proposal);
                    let head = &self.head;
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
                        let block_id = new_proposal.block_id.clone();
                        let pred_hash = new_proposal.pred_hash;
                        actions.extend(Self::preendorse(block_id, pred_hash));
                    }
                    self.head = new_proposal;
                } else {
                    log::warn!(" .  ignore proposal");
                    return ArrayVec::default();
                }
            }
        };
        self.incomplete_prequorum = Votes::default();
        self.incomplete_quorum = Votes::default();
        self.payload = P::EMPTY;

        self.cache.entry(self.head.hash).or_insert_with(|| Pred {
            timestamp: self.head.timestamp,
            level: self.head.block_id.level,
            round: self.head.block_id.round,
            transition: false,
        });

        let timeout = self
            .update_timeout_this_level(config, map, now)
            .map(Action::ScheduleTimeout);
        actions.extend(timeout);

        actions
    }

    fn update_endorsable_payload(&mut self, new_proposal: &BlockInfo<Id, P>) {
        if let Some(ref new) = &new_proposal.prequorum {
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
                pred_timestamp: self.pred.timestamp,
                pred_round: self.pred.round,
                pred_hash: new_proposal.pred_hash,
                prequorum: new.clone(),
                // put it back
                locked,
            });
        }
    }

    fn preendorse(block_id: BlockId, pred_hash: [u8; 32]) -> Option<Action<Id, P>> {
        log::info!(" .  inject preendorsement: {block_id}");
        Some(Action::Preendorse {
            pred_hash,
            block_id,
        })
    }

    fn preendorsement<V>(
        &mut self,
        config: &Config,
        map: &V,
        now: Timestamp,
        content: Preendorsement<Id>,
        op: P::Item,
    ) -> ArrayVec<Action<Id, P>, 2>
    where
        V: ValidatorMap<Id = Id>,
    {
        let current_round = self.pred.round_local_coord(config, now);

        let head = &self.head;
        if head.block_id == content.block_id && head.block_id.round == current_round {
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
            pred_timestamp: self.pred.timestamp,
            pred_round: self.pred.round,
            pred_hash: self.head.pred_hash,
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

        let endorsement = Self::endorse(head.block_id.clone(), self.head.pred_hash);
        let timeout = self
            .update_timeout_this_level(config, map, now)
            .map(Action::ScheduleTimeout);
        actions.extend(endorsement);
        actions.extend(timeout);

        actions
    }

    fn endorse(block_id: BlockId, pred_hash: [u8; 32]) -> Option<Action<Id, P>> {
        log::info!(" .  inject endorsement: {block_id}");
        Some(Action::Endorse {
            pred_hash,
            block_id,
        })
    }

    fn endorsement<V>(
        &mut self,
        config: &Config,
        map: &V,
        now: Timestamp,
        content: Endorsement<Id>,
        op: P::Item,
    ) -> ArrayVec<Action<Id, P>, 2>
    where
        V: ValidatorMap<Id = Id>,
    {
        let current_round = self.pred.round_local_coord(config, now);

        let head = &self.head;
        if head.block_id == content.block_id && head.block_id.round == current_round {
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
        V: ValidatorMap<Id = Id>,
    {
        if let Some(ref elected_block) = self.elected_block {
            let elected_round = elected_block.head.block_id.round;
            let elected_duration = config.round_duration(elected_round);
            let start_next_level = elected_block.head.timestamp + elected_duration;
            let round = config.round(now, start_next_level);
            if let Some((round, proposer)) = map.proposer(self.head.block_id.level + 1, round) {
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
                self.closest_round = ClosestRound::NextLevel { proposer, round, timestamp };
                return Some(timestamp);
            }
        }

        None
    }

    // call when endorsable payload changed
    fn update_timeout_this_level<V>(
        &mut self,
        config: &Config,
        map: &V,
        now: Timestamp,
    ) -> Option<Timestamp>
    where
        V: ValidatorMap<Id = Id>,
    {
        // let current_round = self.round_local_coord(config, now);
        let (pred_timestamp, pred_round) = self
            .endorsable_payload
            .as_ref()
            .map(|endorsable| (endorsable.pred_timestamp, endorsable.pred_round))
            .unwrap_or((self.pred.timestamp, self.pred.round));
        let start_this_level = pred_timestamp + config.round_duration(pred_round);
        let round = config.round(now, start_this_level);

        if let Some((round, proposer)) = map.proposer(self.head.block_id.level, round + 1) {
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
            self.closest_round = ClosestRound::ThisLevel { proposer, round, timestamp };
            return Some(timestamp);
        }

        None
    }
}
