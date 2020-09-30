// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use crypto::{crypto_box::random_keypair, hash::HashType, proof_of_work::ProofOfWork};

pub type IdentitySerdeError = serde_json::Error;

/// This node identity information compatible with Tezos
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

    pub fn from_json(json: &str) -> Result<Identity, IdentitySerdeError> {
        serde_json::from_str::<Identity>(json)
    }

    pub fn as_json(&self) -> Result<String, IdentitySerdeError> {
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity() -> Result<(), failure::Error> {
        // generate
        let identity = Identity::generate(16f64);

        // check
        assert!(!identity.peer_id.is_empty());
        assert!(!identity.public_key.is_empty());
        assert!(!identity.secret_key.is_empty());
        assert!(!identity.proof_of_work_stamp.is_empty());

        // convert json and back
        let converted = identity.as_json()?;
        let converted = Identity::from_json(&converted)?;

        assert_eq!(identity.peer_id, converted.peer_id);
        assert_eq!(identity.public_key, converted.public_key);
        assert_eq!(identity.secret_key, converted.secret_key);
        assert_eq!(identity.proof_of_work_stamp, converted.proof_of_work_stamp);

        Ok(())
    }
}
