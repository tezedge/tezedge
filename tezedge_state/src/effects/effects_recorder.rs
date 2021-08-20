// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, VecDeque};
use std::fmt::{self, Debug};

use crypto::nonce::Nonce;

use crate::peer_address::{PeerAddress, PeerListenerAddress};
use crate::Effects;

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Default, Clone)]
pub struct RecordedEffects {
    nonces: VecDeque<Nonce>,
    chosen_peers_to_connect_to: VecDeque<Vec<PeerListenerAddress>>,
    chosen_potential_peers_for_advertise: VecDeque<Vec<PeerListenerAddress>>,
    chosen_potential_peers_for_nack: VecDeque<Vec<PeerListenerAddress>>,
}

impl RecordedEffects {
    pub fn new() -> Self {
        Default::default()
    }
}

impl Effects for RecordedEffects {
    fn get_nonce(&mut self, _: &PeerAddress) -> Nonce {
        self.nonces
            .pop_front()
            .expect("no more recorded nonces avaiable")
    }

    fn choose_peers_to_connect_to(
        &mut self,
        _: &BTreeSet<PeerListenerAddress>,
        _: usize,
    ) -> Vec<PeerListenerAddress> {
        self.chosen_peers_to_connect_to
            .pop_front()
            .expect("no more recorded chosen peers to connect to avaiable")
    }

    fn choose_potential_peers_for_advertise(
        &mut self,
        _: &BTreeSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress> {
        self.chosen_potential_peers_for_advertise
            .pop_front()
            .expect("no more recorded chosen potential peers for advertise avaiable")
    }

    fn choose_potential_peers_for_nack(
        &mut self,
        _: &BTreeSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress> {
        self.chosen_potential_peers_for_nack
            .pop_front()
            .expect("no more recorded chosen potential peers for nack avaiable")
    }
}

pub struct EffectsRecorder<'a, Efs> {
    effects: &'a mut Efs,
    recorded: RecordedEffects,
}

impl<'a, Efs> EffectsRecorder<'a, Efs> {
    pub fn new(effects: &'a mut Efs) -> Self {
        Self {
            effects,
            recorded: RecordedEffects::new(),
        }
    }

    pub fn record(&mut self) -> &mut Self {
        self
    }

    pub fn finish_recording(self) -> RecordedEffects {
        self.recorded
    }
}

impl<'a, Efs> Debug for EffectsRecorder<'a, Efs>
where
    Efs: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EffectsRecorder")
            .field("effects", &self.effects)
            .field("recorded", &self.recorded)
            .finish()
    }
}

impl<'a, Efs> Effects for EffectsRecorder<'a, Efs>
where
    Efs: Effects + Debug,
{
    fn get_nonce(&mut self, peer: &PeerAddress) -> Nonce {
        let nonce = self.effects.get_nonce(peer);
        self.recorded.nonces.push_back(nonce.clone());
        nonce
    }

    fn choose_peers_to_connect_to(
        &mut self,
        potential_peers: &BTreeSet<PeerListenerAddress>,
        choice_len: usize,
    ) -> Vec<PeerListenerAddress> {
        let addrs = self
            .effects
            .choose_peers_to_connect_to(potential_peers, choice_len);
        self.recorded
            .chosen_peers_to_connect_to
            .push_back(addrs.clone());
        addrs
    }

    fn choose_potential_peers_for_advertise(
        &mut self,
        potential_peers: &BTreeSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress> {
        let addrs = self
            .effects
            .choose_potential_peers_for_advertise(potential_peers);
        self.recorded
            .chosen_potential_peers_for_advertise
            .push_back(addrs.clone());
        addrs
    }

    fn choose_potential_peers_for_nack(
        &mut self,
        potential_peers: &BTreeSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress> {
        let addrs = self
            .effects
            .choose_potential_peers_for_nack(potential_peers);
        self.recorded
            .chosen_potential_peers_for_nack
            .push_back(addrs.clone());
        addrs
    }
}
