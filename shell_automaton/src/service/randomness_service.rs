use std::{fmt::Debug, net::SocketAddr};

use crypto::nonce::Nonce;
use rand::seq::SliceRandom;
use rand::Rng;

pub type RandomnessServiceDefault = rand::prelude::StdRng;

pub trait RandomnessService {
    fn get_nonce(&mut self, peer: SocketAddr) -> Nonce;

    /// Choose peer to initiate random outgoing connection.
    fn choose_peer(&mut self, list: &[SocketAddr]) -> Option<SocketAddr>;
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
}
