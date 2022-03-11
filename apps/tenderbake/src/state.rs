// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{cmp::Ordering, mem, time::Duration};

use super::{
    block::{BlockId, BlockInfo, Payload, Prequorum, Quorum, Votes},
    interface::{Action, Config, Endorsement, Event, Preendorsement, Proposal},
    timestamp::Timestamp,
    validator::ValidatorMap,
};

#[derive(Debug)]
pub struct EndorsablePayload<P>
where
    P: Payload,
{
    pred: Option<BlockInfo<P>>,
    prequorum: Prequorum<P::Item>,
}

#[derive(Debug)]
pub struct ElectedBlock<P>
where
    P: Payload,
{
    head: BlockInfo<P>,
    quorum: Quorum<P::Item>,
}

#[derive(Debug)]
pub struct Machine<P>
where
    P: Payload,
{
    proposal: Proposal<P>,
    incomplete_prequorum: Votes<P::Item>,
    incomplete_quorum: Votes<P::Item>,

    locked_round: Option<BlockId>,
    endorsable_payload: Option<EndorsablePayload<P>>,
    elected_block: Option<ElectedBlock<P>>,
    round_state: i32,
    payload: P,
}

impl<P> Default for Machine<P>
where
    P: Payload,
{
    fn default() -> Self {
        Machine {
            proposal: Proposal {
                pred: BlockInfo::GENESIS,
                head: BlockInfo::GENESIS,
            },
            incomplete_prequorum: Votes::default(),
            incomplete_quorum: Votes::default(),
            locked_round: None,
            endorsable_payload: None,
            elected_block: None,
            round_state: 0,
            payload: P::EMPTY,
        }
    }
}

impl<P> Machine<P>
where
    P: Payload + Clone + core::fmt::Debug,
    P::Item: Clone + core::fmt::Debug,
{
    pub fn handle<V>(&mut self, config: &Config, map: &V, event: Event<P>) -> Vec<Action<P>>
    where
        V: ValidatorMap,
    {
        match event {
            Event::Timeout(now) => self.timeout(config, map, now),
            Event::TimeoutDelayed(now) => self.timeout_delayed(config, now),
            Event::Proposal(proposal, now) => self.proposal(config, map, *proposal, now),
            Event::Preendorsement(preendorsement, op) => self
                .preendorsement(config, map, preendorsement, op)
                .map(|(content, pred_hash)| Action::Endorse { pred_hash, content })
                .into_iter()
                .collect(),
            Event::Endorsement(endorsement, op) => {
                self.endorsement(config, endorsement, op);
                Vec::default()
            }
            Event::PayloadItem(item) => {
                self.payload.update(item);
                Vec::default()
            }
        }
    }

    fn timeout<V>(&mut self, config: &Config, map: &V, now: Timestamp) -> Vec<Action<P>>
    where
        V: ValidatorMap,
    {
        log::debug!("State: {self:#?}");
        log::info!("Timeout: {now}");

        let proposal = &self.proposal;
        let round_delta = proposal.pred.block_id.round
            - self
                .endorsable_payload
                .as_ref()
                .map(|pqc| pqc.prequorum.block_id.round)
                .unwrap_or(0);
        log::info!(" .  round delta: {round_delta}");
        let next_round = self.round_state + 1 + round_delta;
        let is_proposer_next_round = map.proposer(proposal.head.block_id.level, next_round);

        if let Some(ref elected_block) = &self.elected_block {
            let elected_round = elected_block.head.block_id.round;
            let this_round = self.round_state - elected_round;
            if map.proposer(proposal.head.block_id.level + 1, this_round) {
                log::info!(
                    " .  has proposer slot at {}:{}",
                    proposal.head.block_id.level + 1,
                    this_round
                );
                let new_proposal = Proposal {
                    pred: elected_block.head.clone(),
                    head: BlockInfo {
                        hash: [0; 32],
                        block_id: BlockId {
                            level: proposal.head.block_id.level + 1,
                            round: this_round,
                            payload_hash: [0; 32],
                            payload_round: this_round,
                        },
                        timestamp: now,
                        transition: false,
                        prequorum: None,
                        quorum: Some(elected_block.quorum.clone()),
                        payload: mem::replace(&mut self.payload, P::EMPTY),
                    },
                };
                let mut actions = Vec::new();
                actions.push(Action::Propose(Box::new(new_proposal)));

                self.round_state += 1;
                let next_timeout = now + config.round_duration(self.round_state);
                actions.push(Action::ScheduleTimeout(next_timeout));

                return actions;
            } else {
                log::info!(
                    " .  has *no* proposer slot at {}:{}",
                    proposal.head.block_id.level + 1,
                    this_round
                );
            }
        }

        if is_proposer_next_round {
            log::info!(
                " .  has proposer slot at {}:{}",
                proposal.head.block_id.level,
                next_round
            );

            let mut actions = Vec::new();
            let delay = config.minimal_block_delay / 5;
            actions.push(Action::ScheduleTimeoutDelayed(delay));
            return actions;
        }
        log::info!(
            " .  has *no* proposer slot at {}:{}",
            proposal.head.block_id.level,
            next_round
        );

        let mut actions = Vec::new();

        self.round_state += 1;
        let next_timeout = now + config.round_duration(self.round_state);
        actions.push(Action::ScheduleTimeout(next_timeout));

        actions
    }

    fn timeout_delayed(&mut self, config: &Config, now: Timestamp) -> Vec<Action<P>> {
        log::info!("Timeout delayed: {now}");

        let timestamp = now - config.minimal_block_delay / 5;

        let proposal = &self.proposal;

        let mut actions = Vec::new();
        let mut pred = proposal.pred.clone();
        let mut head = proposal.head.clone();
        head.timestamp = timestamp;
        head.block_id.round = self.round_state + 1;
        if let Some(ref endorsable_payload) = &self.endorsable_payload {
            head.block_id.payload_hash = endorsable_payload.prequorum.block_id.payload_hash;
            head.block_id.payload_round = endorsable_payload.prequorum.block_id.payload_round;
            head.prequorum = Some(endorsable_payload.prequorum.clone());
            pred = endorsable_payload
                .pred
                .as_ref()
                .cloned()
                .unwrap_or(BlockInfo::GENESIS);
        } else {
            head.block_id.payload_hash = [0; 32];
            head.block_id.payload_round = self.round_state;
            head.prequorum = None;
        }
        actions.push(Action::Propose(Box::new(Proposal { pred, head })));

        self.round_state += 1;
        let next_timeout = timestamp + config.round_duration(self.round_state);
        actions.push(Action::ScheduleTimeout(next_timeout));
        actions
    }

    fn proposal<V>(
        &mut self,
        config: &Config,
        map: &V,
        new_proposal: Proposal<P>,
        now: Timestamp,
    ) -> Vec<Action<P>>
    where
        V: ValidatorMap,
    {
        log::debug!("State: {self:#?}");
        log::info!("Proposal: {:?}", new_proposal.head);

        let Machine {
            ref mut proposal,
            ref mut incomplete_prequorum,
            ref mut incomplete_quorum,
            ref mut locked_round,
            ref mut endorsable_payload,
            ref mut elected_block,
            ref mut round_state,
            ref mut payload,
        } = self;

        let mut actions = Vec::new();
        let new_state = match proposal
            .head
            .block_id
            .level
            .cmp(&new_proposal.head.block_id.level)
        {
            Ordering::Less => {
                // we received a block for a next level
                let (this_round, next_timeout) = match &new_proposal.head.transition {
                    true => (0, new_proposal.head.timestamp + config.round_duration(0)),
                    false => {
                        let start_of_this_level = start_by_pred(config, &new_proposal.pred);
                        log::info!(" .  calculate timeout, level started: {start_of_this_level}");
                        our_round(config, now, start_of_this_level)
                    }
                };
                log::info!(
                    " .  calculate timeout, this round: {this_round}, next timeout: {next_timeout}"
                );
                actions.push(Action::ScheduleTimeout(next_timeout));

                *round_state = this_round;

                *locked_round = None;
                *endorsable_payload =
                    new_proposal
                        .head
                        .prequorum
                        .as_ref()
                        .map(|prequorum| EndorsablePayload {
                            pred: Some(new_proposal.pred.clone()),
                            prequorum: prequorum.clone(),
                        });
                *elected_block = None;

                if *round_state < new_proposal.head.block_id.round {
                    // proposal from future, ignore
                    log::info!(" .  ignore proposal from future");
                    let _ = new_proposal;
                    return Vec::default();
                } else {
                    if *round_state == new_proposal.head.block_id.round {
                        let block_id = new_proposal.head.block_id.clone();
                        log::info!(" .  inject preendorsement: {block_id}");
                        if let Some(validator) = map.preendorser(block_id.level, block_id.round) {
                            log::info!(" .  has slot");
                            actions.push(Action::Preendorse {
                                pred_hash: new_proposal.pred.hash,
                                content: Preendorsement {
                                    validator,
                                    block_id,
                                },
                            });
                        } else {
                            log::info!(" .  has *no* slot");
                        }
                    }

                    new_proposal
                }
            }
            Ordering::Greater => {
                // The baker is ahead, a reorg may have happened. Do nothing:
                // wait for the node to send us the branch's head. This new head
                // should have a fitness that is greater than our current
                // proposal and thus, its level should be at least the same as
                // our current proposal's level.
                return Vec::default();
            }
            Ordering::Equal => {
                let Proposal {
                    ref mut head,
                    ref pred,
                } = proposal;

                let new_block = new_proposal.head;
                let new_pred = new_proposal.pred;

                if pred.hash != new_pred.hash {
                    let switch = match (&*endorsable_payload, &head.prequorum) {
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
                        log::info!(" .  ignore proposal, will not change branch");
                        return Vec::default();
                    }
                }

                if new_block.block_id.round < *round_state
                    && head.block_id.round < new_block.block_id.round
                {
                    if new_block.prequorum > head.prequorum {
                        head.prequorum = new_block.prequorum.clone();
                    }
                    Proposal {
                        pred: new_pred,
                        head: new_block,
                    }
                } else if new_block.block_id.round == *round_state
                    && (head.block_id.round != new_block.block_id.round
                        || head.block_id.payload_hash == new_block.block_id.payload_hash)
                {
                    if new_block.prequorum > head.prequorum {
                        head.prequorum = new_block.prequorum.clone();
                    }
                    let will_preendorse = match &*locked_round {
                        None => true,
                        Some(ref locked_round) => {
                            matches!(
                                &head.prequorum,
                                Some(ref prequorum) if prequorum.block_id.round >= locked_round.round,
                            ) || locked_round.payload_hash == head.block_id.payload_hash
                        }
                    };
                    if will_preendorse {
                        let block_id = new_block.block_id.clone();
                        log::info!(" .  inject preendorsement: {block_id}");
                        if let Some(validator) = map.preendorser(block_id.level, block_id.round) {
                            log::info!(" .  has slot");
                            actions.push(Action::Preendorse {
                                pred_hash: new_pred.hash,
                                content: Preendorsement {
                                    validator,
                                    block_id,
                                },
                            });
                        } else {
                            log::info!(" .  has *no* slot");
                        }
                    }
                    Proposal {
                        pred: new_pred,
                        head: new_block,
                    }
                } else {
                    log::info!(" .  ignore proposal");
                    return Vec::default();
                }
            }
        };
        if new_state.head.block_id != proposal.head.block_id {
            *incomplete_prequorum = Votes::default();
            *incomplete_quorum = Votes::default();
            *payload = P::EMPTY;
        }
        *proposal = new_state;

        actions
    }

    fn preendorsement<V>(
        &mut self,
        config: &Config,
        map: &V,
        preendorsement: Preendorsement,
        op: P::Item,
    ) -> Option<(Endorsement, [u8; 32])>
    where
        V: ValidatorMap,
    {
        log::debug!("Preendorsement: {preendorsement}");

        let head = &self.proposal.head;
        if head.block_id == preendorsement.block_id && head.block_id.round == self.round_state {
            if self
                .incomplete_prequorum
                .ids
                .insert(preendorsement.validator.id, op)
                .is_none()
            {
                self.incomplete_prequorum.power += preendorsement.validator.power;
            }
            log::debug!(" .  accept preendorsement");
        }

        if self.incomplete_prequorum.power >= config.consensus_threshold {
            log::debug!(" .  reach preendorsement consensus");
            match &self.locked_round {
                Some(ref locked) if locked.round == head.block_id.round => {
                    log::debug!(" .  already has locked {locked:?}");
                }
                _ => {
                    self.endorsable_payload = Some(EndorsablePayload {
                        pred: Some(self.proposal.pred.clone()),
                        prequorum: Prequorum {
                            block_id: head.block_id.clone(),
                            votes: self.incomplete_prequorum.clone(),
                        },
                    });
                    log::info!(" .  lock {:?}", head.block_id);
                    self.locked_round = Some(head.block_id.clone());
                    log::info!(" .  inject endorsement: {}", head.block_id);
                    return map
                        .endorser(head.block_id.level, head.block_id.round)
                        .or_else(|| {
                            log::info!(" .  has *no* slot");
                            None
                        })
                        .map(|validator| {
                            log::info!(" .  has slot");
                            (
                                Endorsement {
                                    validator,
                                    block_id: head.block_id.clone(),
                                },
                                self.proposal.pred.hash,
                            )
                        });
                }
            }
        }

        None
    }

    fn endorsement(&mut self, config: &Config, endorsement: Endorsement, op: P::Item) {
        log::debug!("Endorsement: {endorsement}");

        let head = &self.proposal.head;

        if head.block_id == endorsement.block_id && head.block_id.round == self.round_state {
            if self
                .incomplete_quorum
                .ids
                .insert(endorsement.validator.id, op)
                .is_none()
            {
                self.incomplete_quorum.power += endorsement.validator.power;
            }
            log::debug!(" .  accept endorsement {}", self.incomplete_quorum.power);
        }

        if self.incomplete_quorum.power >= config.consensus_threshold {
            log::debug!(" .  reach endorsement consensus");
            if let Some(ref elected_block) = &self.elected_block {
                log::debug!(" .  already has elected {:?}", elected_block.head);
            } else {
                let elected = ElectedBlock {
                    head: head.clone(),
                    quorum: Quorum {
                        votes: self.incomplete_quorum.clone(),
                    },
                };
                log::info!(" .  elected {elected:?}");
                self.elected_block = Some(elected);
            }
        }
    }
}

fn start_by_pred<P>(config: &Config, pred: &BlockInfo<P>) -> Timestamp
where
    P: Payload,
{
    let duration = config.round_duration(pred.block_id.round);
    pred.timestamp + duration
}

fn our_round(config: &Config, now: Timestamp, start_of_this_level: Timestamp) -> (i32, Timestamp) {
    let elapsed = if now < start_of_this_level {
        Duration::ZERO
    } else {
        now - start_of_this_level
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
    let d = config.delay_increment_per_round.as_secs_f64();
    let m = config.minimal_block_delay.as_secs_f64();
    let p = d - 2.0 * m;
    let r = (p + (p * p + 8.0 * d * e).sqrt()) / (2.0 * d);
    let r = r.floor();

    let t = start_of_this_level + Duration::from_secs_f64(m * (r + 1.0) + d * (r + 1.0) * r / 2.0);

    (r as i32, t)
}

#[cfg(test)]
mod tests {
    use core::time::Duration;

    use super::{our_round, Config, Timestamp};

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
        let (r, _) = our_round(&config, now, start_of_this_level);
        assert_eq!(r, 2);
    }
}
