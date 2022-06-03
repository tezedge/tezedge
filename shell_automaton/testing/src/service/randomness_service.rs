// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::net::SocketAddr;

use crypto::nonce::{Nonce, NONCE_SIZE};
use shell_automaton::baker::seed_nonce::{SeedNonce, SeedNonceHash};
use shell_automaton::service::RandomnessService;
use tezos_messages::p2p::encoding::block_header::Level;

#[derive(Debug, Clone)]
pub enum RandomnessServiceMocked {
    Dummy,
}

impl RandomnessService for RandomnessServiceMocked {
    fn get_nonce(&mut self, _: SocketAddr) -> Nonce {
        match self {
            Self::Dummy => Nonce::new(&[0; NONCE_SIZE]),
        }
    }

    fn choose_peer(&mut self, list: &[SocketAddr]) -> Option<SocketAddr> {
        match self {
            Self::Dummy => list.get(0).cloned(),
        }
    }

    fn choose_potential_peers_for_advertise(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr> {
        match self {
            Self::Dummy => list.iter().cloned().take(80).collect(),
        }
    }

    fn choose_potential_peers_for_nack(&mut self, list: &[SocketAddr]) -> Vec<SocketAddr> {
        self.choose_potential_peers_for_advertise(list)
    }

    fn get_seed_nonce(&mut self, _level: Level) -> (SeedNonceHash, SeedNonce) {
        match self {
            Self::Dummy => todo!(),
        }
    }
}
