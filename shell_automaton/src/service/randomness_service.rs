use std::{fmt::Debug, net::SocketAddr};

use crypto::nonce::Nonce;
use rand::seq::SliceRandom;
use rand::Rng;

pub type RandomnessServiceDefault = rand::prelude::StdRng;

pub trait RandomnessService {
    fn get_nonce(&mut self, peer: SocketAddr) -> Nonce;

    /// Choose peer to initiate random outgoing connection.
    fn choose_peer(&mut self, list: &[SocketAddr]) -> Option<SocketAddr>;

    fn choose_potential_peers_for_advertise(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr>;

    fn choose_potential_peers_for_nack(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr>;
}

impl<R> RandomnessService for R
where
    R: Rng + Debug,
{
    fn get_nonce(&mut self, _: SocketAddr) -> Nonce {
        let mut b = [0; 24];
        self.fill(&mut b);
        Nonce::new(&b)
    }

    fn choose_peer(&mut self, list: &[SocketAddr]) -> Option<SocketAddr> {
        list.choose(self).cloned()
    }

    fn choose_potential_peers_for_advertise(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr> {
        let len = self.gen_range(1, 80.min(list.len()).max(2));
        if len >= list.len() {
            list.iter().cloned().collect()
        } else {
            list.choose_multiple(self, len).cloned().collect()
        }
    }

    fn choose_potential_peers_for_nack(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr> {
        self.choose_potential_peers_for_advertise(list)
    }
}
