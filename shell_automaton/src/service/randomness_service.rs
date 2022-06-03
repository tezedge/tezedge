// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{fmt::Debug, net::SocketAddr};

use crypto::nonce::{Nonce, NONCE_SIZE};
use rand::seq::SliceRandom;
use rand::Rng;
use tezos_messages::p2p::encoding::block_header::Level;

use crate::baker::seed_nonce::{SeedNonce, SeedNonceHash};
use crypto::blake2b;

pub type RandomnessServiceDefault = rand::prelude::StdRng;

pub trait RandomnessService {
    fn get_nonce(&mut self, peer: SocketAddr) -> Nonce;

    /// Choose peer to initiate random outgoing connection.
    fn choose_peer(&mut self, list: &[SocketAddr]) -> Option<SocketAddr>;

    fn choose_potential_peers_for_advertise(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr>;

    fn choose_potential_peers_for_nack(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr>;

    fn get_seed_nonce(&mut self, level: Level) -> (SeedNonceHash, SeedNonce);
}

impl<R> RandomnessService for R
where
    R: Rng + Debug,
{
    fn get_nonce(&mut self, _: SocketAddr) -> Nonce {
        let mut b: [u8; NONCE_SIZE] = [0; NONCE_SIZE];
        self.fill(&mut b);
        Nonce::new(&b)
    }

    fn choose_peer(&mut self, list: &[SocketAddr]) -> Option<SocketAddr> {
        list.choose(self).cloned()
    }

    fn choose_potential_peers_for_advertise(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr> {
        let len = self.gen_range(1, 80.min(list.len()).max(2));
        if len >= list.len() {
            list.to_vec()
        } else {
            list.choose_multiple(self, len).cloned().collect()
        }
    }

    fn choose_potential_peers_for_nack(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr> {
        self.choose_potential_peers_for_advertise(list)
    }

    fn get_seed_nonce(&mut self, _level: Level) -> (SeedNonceHash, SeedNonce) {
        let mut nonce_bytes = [0; 32];
        self.fill(&mut nonce_bytes);
        let hash_bytes = blake2b::digest_256(&nonce_bytes).unwrap();
        let hash = SeedNonceHash::try_from(hash_bytes).unwrap();
        (hash, nonce_bytes.into())
    }
}
