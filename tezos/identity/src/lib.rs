// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT
#![forbid(unsafe_code)]

use std::collections::HashMap;

use failure::Fail;
use hex::FromHexError;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crypto::{crypto_box::random_keypair, hash::HashType, proof_of_work::ProofOfWork};
use crypto::hash::CryptoboxPublicKeyHash;

#[derive(Fail, Debug)]
pub enum IdentityError {
    #[fail(display = "Serde error, reason: {}", reason)]
    IdentitySerdeError { reason: serde_json::Error },

    #[fail(display = "Invalid field error, reason: {}", reason)]
    IdentityFieldError { reason: String },

    #[fail(display = "Invalid public key, reason: {}", reason)]
    InvalidPublicKeyError { reason: FromHexError },

    #[fail(display = "Identity invalid's peer_id check, reason: {}", reason)]
    InvalidPeerIdError { reason: String },
}

/// This node identity information compatible with Tezos
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Identity {
    /// Peer_id is calculated hash of public_key [`crypto_box::PublicKey`]
    peer_id: CryptoboxPublicKeyHash,

    // TODO: TE-217 - refactor to real types
    /// Hex encoded public key: [`crypto_box::PublicKey`]
    pub public_key: String,
    /// Hex encoded secret key: [`crypto_box::SecretKey`]
    pub secret_key: String,
    /// Hex encoded pow: [`crypto::ProofOfWork`]
    pub proof_of_work_stamp: String,
}

impl Identity {
    pub fn generate(expected_pow: f64) -> Self {
        let (sk, pk, peer_id) = random_keypair();
        let pow = ProofOfWork::generate(&pk, expected_pow);
        Identity {
            peer_id,
            public_key: hex::encode(pk.as_ref()),
            secret_key: hex::encode(sk.as_ref()),
            proof_of_work_stamp: hex::encode(pow.as_ref()),
        }
    }

    // TODO: TE-217 - fix generate identity/peer_id (remove: validate_peer_id - always validate)
    pub fn from_json(json: &str, validate_peer_id: bool) -> Result<Identity, IdentityError> {
        let identity: HashMap<String, Value> = serde_json::from_str(json)
            .map_err(|e| IdentityError::IdentitySerdeError { reason: e })?;

        let peer_id = match identity.get("peer_id") {
            Some(peer_id) => match peer_id.as_str() {
                Some(peer_id) => peer_id.to_string(),
                None => return Err(IdentityError::IdentityFieldError { reason: "Missing valid 'peer_id'".to_string() })
            },
            None => return Err(IdentityError::IdentityFieldError { reason: "Missing 'peer_id'".to_string() })
        };
        let public_key = match identity.get("public_key") {
            Some(public_key) => match public_key.as_str() {
                Some(public_key) => public_key.to_string(),
                None => return Err(IdentityError::IdentityFieldError { reason: "Missing valid 'public_key'".to_string() })
            },
            None => return Err(IdentityError::IdentityFieldError { reason: "Missing 'public_key'".to_string() })
        };
        let secret_key = match identity.get("secret_key") {
            Some(secret_key) => match secret_key.as_str() {
                Some(secret_key) => secret_key.to_string(),
                None => return Err(IdentityError::IdentityFieldError { reason: "Missing valid 'secret_key'".to_string() })
            },
            None => return Err(IdentityError::IdentityFieldError { reason: "Missing 'public_key'".to_string() })
        };
        let proof_of_work_stamp = match identity.get("proof_of_work_stamp") {
            Some(proof_of_work_stamp) => match proof_of_work_stamp.as_str() {
                Some(proof_of_work_stamp) => proof_of_work_stamp.to_string(),
                None => return Err(IdentityError::IdentityFieldError { reason: "Missing valid 'proof_of_work_stamp'".to_string() })
            },
            None => return Err(IdentityError::IdentityFieldError { reason: "Missing 'proof_of_work_stamp'".to_string() })
        };

        // check peer_id
        let calculated_peer_id = {
            // TODO: TE-217 - &public_key is CryptoboxPublicKeyHash or [`crypto_box::PublicKey`] ?
            let public_key_hash: CryptoboxPublicKeyHash = Self::calculate_peer_id(&public_key)?;
            if validate_peer_id && !peer_id.eq(&HashType::CryptoboxPublicKeyHash.bytes_to_string(&public_key_hash)) {
                return Err(IdentityError::InvalidPeerIdError { reason: "Invalid peer_id".to_string() });
            }
            public_key_hash
        };

        Ok(
            Identity {
                peer_id: calculated_peer_id,
                public_key,
                secret_key,
                proof_of_work_stamp,
            }
        )
    }

    pub fn as_json(&self) -> Result<String, IdentityError> {
        let mut identity: HashMap<&'static str, String> = Default::default();
        identity.insert("peer_id", HashType::CryptoboxPublicKeyHash.bytes_to_string(&self.peer_id));
        identity.insert("public_key", self.public_key.clone());
        identity.insert("secret_key", self.secret_key.clone());
        identity.insert("proof_of_work_stamp", self.proof_of_work_stamp.clone());
        serde_json::to_string(&identity)
            .map_err(|e| IdentityError::IdentitySerdeError { reason: e })
    }

    // TODO: TE-217 - does not needed maybe?
    pub fn calculated_peer_id(&self) -> Result<CryptoboxPublicKeyHash, IdentityError> {
        Self::calculate_peer_id(&self.public_key)
    }

    fn calculate_peer_id(public_key_as_hex_string: &str) -> Result<CryptoboxPublicKeyHash, IdentityError> {
        hex::decode(&public_key_as_hex_string).map_err(|e| IdentityError::InvalidPeerIdError { reason: format!("{}", e) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_generate() -> Result<(), failure::Error> {
        // generate
        let identity = Identity::generate(16f64);
        // TODO: TE-217 - fix generate identity/peer_id (remove: validate_peer_id - always validate)
        let validate_peer_id = false;

        // check
        assert!(!identity.peer_id.is_empty());
        assert!(!identity.public_key.is_empty());
        assert!(!identity.secret_key.is_empty());
        assert!(!identity.proof_of_work_stamp.is_empty());

        // convert json and back
        let converted = identity.as_json()?;
        let converted = Identity::from_json(&converted, validate_peer_id)?;

        if validate_peer_id {
            assert_eq!(identity.peer_id, converted.peer_id);
        }
        assert_eq!(identity.public_key, converted.public_key);
        assert_eq!(identity.secret_key, converted.secret_key);
        assert_eq!(identity.proof_of_work_stamp, converted.proof_of_work_stamp);

        Ok(())
    }

    #[test]
    fn test_identity_json_serde_generated_by_tezos() -> Result<(), failure::Error> {
        let expected_json = serde_json::json!(
            {
              "peer_id": "idtqxHUjbjbCfaDn4jczoPGsnhacKX",
              "public_key": "a072c7b3e477142689cadee638078b377df5e5793e3cea529d0b718cde59f212",
              "secret_key": "d37c77a8643c7f7fce9219e9769ed4dd23bc542265da47a64a2613bd199ad74e",
              "proof_of_work_stamp": "0cfe810d9b4591f0f50721b6811f2981a4274e9d0593bbd0"
            }
        );

        let converted = Identity::from_json(serde_json::to_string(&expected_json)?.as_str(), true)?;
        let converted = converted.as_json()?;
        let converted = Identity::from_json(&converted, true)?;
        let converted = converted.as_json()?;

        // check
        assert_json_diff::assert_json_eq!(
            serde_json::from_str::<serde_json::Value>(&converted)?,
            expected_json
        );
        Ok(())
    }
}
