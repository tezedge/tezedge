// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use crypto::{crypto_box::random_keypair, hash::HashType, proof_of_work::ProofOfWork};

/// This node identity information
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Identity {
    pub peer_id: String,
    pub public_key: String,
    pub secret_key: String,
    pub proof_of_work_stamp: String,
}

impl Identity {
    pub fn generate(expected_pow: f64) -> Self {
        let (sk, pk, peer_id) = random_keypair();
        let pow = ProofOfWork::generate(&pk, expected_pow);
        Identity {
            peer_id: HashType::CryptoboxPublicKeyHash.bytes_to_string(peer_id.as_ref()),
            public_key: hex::encode(pk.as_ref()),
            secret_key: hex::encode(sk.as_ref()),
            proof_of_work_stamp: hex::encode(pow.as_ref()),
        }
    }
}