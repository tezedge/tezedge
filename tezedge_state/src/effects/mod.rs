use rand::{prelude::IteratorRandom, rngs::ThreadRng, Rng};
use std::collections::HashSet;
use std::fmt::Debug;

use crypto::nonce::Nonce;

use crate::peer_address::{PeerAddress, PeerListenerAddress};

mod effects_recorder;
pub use effects_recorder::*;

/// Effects are source of randomness.
pub trait Effects: Debug {
    fn get_nonce(&mut self, peer: &PeerAddress) -> Nonce;

    fn choose_peers_to_connect_to(
        &mut self,
        potential_peers: &HashSet<PeerListenerAddress>,
        choice_len: usize,
    ) -> Vec<PeerListenerAddress>;

    fn choose_potential_peers_for_advertise(
        &mut self,
        potential_peers: &HashSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress>;

    fn choose_potential_peers_for_nack(
        &mut self,
        potential_peers: &HashSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress>;
}

pub type DefaultEffects = ThreadRng;

impl<R> Effects for R
where
    R: Rng + Debug,
{
    fn get_nonce(&mut self, _: &PeerAddress) -> Nonce {
        let mut b = [0; 24];
        self.fill(&mut b);
        Nonce::new(&b)
    }

    fn choose_peers_to_connect_to(
        &mut self,
        potential_peers: &HashSet<PeerListenerAddress>,
        choice_len: usize,
    ) -> Vec<PeerListenerAddress> {
        if choice_len == 0 {
            return vec![];
        }

        if choice_len >= potential_peers.len() {
            potential_peers.iter().cloned().collect()
        } else {
            potential_peers
                .iter()
                .cloned()
                .choose_multiple(self, choice_len)
        }
    }

    fn choose_potential_peers_for_advertise(
        &mut self,
        potential_peers: &HashSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress> {
        let len = self.gen_range(1, 80.min(potential_peers.len()).max(2));
        if len >= potential_peers.len() {
            potential_peers.iter().cloned().collect()
        } else {
            potential_peers.iter().cloned().choose_multiple(self, len)
        }
    }

    fn choose_potential_peers_for_nack(
        &mut self,
        potential_peers: &HashSet<PeerListenerAddress>,
    ) -> Vec<PeerListenerAddress> {
        self.choose_potential_peers_for_advertise(potential_peers)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_choose_potential_peers_panics() {
        for n in 0..100 {
            let mut p = HashSet::new();
            for i in 0..n {
                p.insert(PeerAddress::ipv4_from_index(i).as_listener_address());
            }
            let mut effects = rand::thread_rng();
            for choice_len in 0..100 {
                effects.choose_peers_to_connect_to(&p, choice_len);
                effects.choose_potential_peers_for_advertise(&p);
                effects.choose_potential_peers_for_nack(&p);
            }
        }
    }
}
