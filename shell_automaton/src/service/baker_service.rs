// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;

use derive_more::From;
use thiserror::Error;

use crypto::hash::{ChainId, SecretKeyEd25519, Signature};
use crypto::CryptoError;
use tezos_encoding::enc::{BinError, BinWriter};
use tezos_messages::base::signature_public_key::SignaturePublicKey;

use crate::baker::block_endorser::{EndorsementWithForgedBytes, PreendorsementWithForgedBytes};

#[derive(Error, From, Debug)]
pub enum EncodeError {
    #[error("{_0}")]
    Bin(BinError),
    #[error("{_0}")]
    Crypto(CryptoError),
}

pub trait BakerService {
    fn preendrosement_sign(
        &mut self,
        baker: &SignaturePublicKey,
        chain_id: &ChainId,
        operation: &PreendorsementWithForgedBytes,
    ) -> Result<Signature, EncodeError>;

    fn endrosement_sign(
        &mut self,
        baker: &SignaturePublicKey,
        chain_id: &ChainId,
        operation: &EndorsementWithForgedBytes,
    ) -> Result<Signature, EncodeError>;
}

pub struct BakerServiceDefault {
    bakers: BTreeMap<SignaturePublicKey, BakerSigner>,
}

pub enum BakerSigner {
    Local { secret_key: SecretKeyEd25519 },
}

impl BakerServiceDefault {
    pub fn new() -> Self {
        Self {
            bakers: Default::default(),
        }
    }

    pub fn add_local_baker(
        &mut self,
        public_key: SignaturePublicKey,
        secret_key: SecretKeyEd25519,
    ) {
        self.bakers
            .insert(public_key, BakerSigner::Local { secret_key });
    }

    pub fn get_baker_mut(&mut self, key: &SignaturePublicKey) -> &mut BakerSigner {
        self.bakers
            .get_mut(key)
            .expect("Missing signed for baker: {key}")
    }

    pub fn sign(
        &mut self,
        baker: &SignaturePublicKey,
        watermark_tag: u8,
        chain_id: &ChainId,
        value_bytes: &[u8],
    ) -> Result<Signature, EncodeError> {
        let watermark_bytes = {
            let mut v = Vec::with_capacity(5);
            v.push(watermark_tag);
            chain_id.bin_write(&mut v)?;
            v
        };
        match self.get_baker_mut(baker) {
            BakerSigner::Local { secret_key, .. } => {
                Ok(secret_key.sign(&[watermark_bytes.as_slice(), value_bytes])?)
            }
        }
    }
}

impl BakerService for BakerServiceDefault {
    fn preendrosement_sign(
        &mut self,
        baker: &SignaturePublicKey,
        chain_id: &ChainId,
        operation: &PreendorsementWithForgedBytes,
    ) -> Result<Signature, EncodeError> {
        self.sign(baker, 0x12, chain_id, operation.forged_with_branch())
    }

    fn endrosement_sign(
        &mut self,
        baker: &SignaturePublicKey,
        chain_id: &ChainId,
        operation: &EndorsementWithForgedBytes,
    ) -> Result<Signature, EncodeError> {
        self.sign(baker, 0x13, chain_id, operation.forged_with_branch())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::hash::Signature;
    use tezos_messages::protocol::proto_012::operation::{
        InlinedEndorsement, InlinedEndorsementMempoolContents,
        InlinedEndorsementMempoolContentsEndorsementVariant, InlinedPreendorsement,
        InlinedPreendorsementContents, InlinedPreendorsementVariant,
    };

    #[test]
    fn test_preendorsement_encoded_in_parts_is_correct() {
        let branch = crypto::hash::BlockHash::from_base58_check(
            "BLq5kHN8jniUaZBGC6krLjGZCWVDD85tTsaHCHS6kEKB5MpX8pA",
        )
        .unwrap();
        let preendorsement = InlinedPreendorsementVariant {
            slot: 10,
            level: 100003,
            round: 2,
            block_payload_hash: crypto::hash::BlockPayloadHash::from_base58_check(
                "vh39tJP8QpsJ2dZgMy28BEDgHF7CbQHo4SX1L2DmjW8GtmH3V9pi",
            )
            .unwrap(),
        };
        let preendorsement = InlinedPreendorsementContents::Preendorsement(preendorsement);
        let preendorsement_full = InlinedPreendorsement {
            branch: branch.clone(),
            operations: preendorsement.clone(),
            signature: Signature(vec![]),
        };

        let mut preendorsement_full_encoded = vec![];
        preendorsement_full
            .bin_write(&mut preendorsement_full_encoded)
            .unwrap();

        let mut preendorsement_encoded = vec![];
        branch.bin_write(&mut preendorsement_encoded).unwrap();
        preendorsement
            .bin_write(&mut preendorsement_encoded)
            .unwrap();

        assert_eq!(preendorsement_encoded, preendorsement_full_encoded);
    }

    #[test]
    fn test_endorsement_encoded_in_parts_is_correct() {
        let branch = crypto::hash::BlockHash::from_base58_check(
            "BLq5kHN8jniUaZBGC6krLjGZCWVDD85tTsaHCHS6kEKB5MpX8pA",
        )
        .unwrap();
        let endorsement = InlinedEndorsementMempoolContentsEndorsementVariant {
            slot: 10,
            level: 100003,
            round: 2,
            block_payload_hash: crypto::hash::BlockPayloadHash::from_base58_check(
                "vh39tJP8QpsJ2dZgMy28BEDgHF7CbQHo4SX1L2DmjW8GtmH3V9pi",
            )
            .unwrap(),
        };
        let endorsement = InlinedEndorsementMempoolContents::Endorsement(endorsement);
        let endorsement_full = InlinedEndorsement {
            branch: branch.clone(),
            operations: endorsement.clone(),
            signature: Signature(vec![]),
        };

        let mut endorsement_full_encoded = vec![];
        endorsement_full
            .bin_write(&mut endorsement_full_encoded)
            .unwrap();

        let mut endorsement_encoded = vec![];
        branch.bin_write(&mut endorsement_encoded).unwrap();
        endorsement.bin_write(&mut endorsement_encoded).unwrap();

        assert_eq!(endorsement_encoded, endorsement_full_encoded);
    }
}
