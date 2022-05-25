// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::convert::TryInto;
use std::sync::{mpsc, Arc};

use derive_more::From;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crypto::blake2b;
use crypto::hash::{ChainId, SecretKeyEd25519, Signature};
use crypto::CryptoError;
use tezos_encoding::enc::{BinError, BinWriter};
use tezos_encoding::types::SizedBytes;
use tezos_messages::base::signature_public_key::SignaturePublicKey;
use tezos_messages::p2p::encoding::block_header::BlockHeader;
use tezos_messages::protocol::proto_012::operation::FullHeader;

use crate::baker::block_endorser::{EndorsementWithForgedBytes, PreendorsementWithForgedBytes};
use crate::request::RequestId;

#[derive(Error, From, Debug)]
pub enum EncodeError {
    #[error("{_0}")]
    Bin(BinError),
    #[error("{_0}")]
    Crypto(CryptoError),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum BakerWorkerMessage {
    ComputeProofOfWork(SizedBytes<8>),
}

pub trait BakerService {
    /// Try to receive/read queued message from baker worker, if there is any.
    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), mpsc::TryRecvError>;

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

    fn block_sign(
        &mut self,
        baker: &SignaturePublicKey,
        chain_id: &ChainId,
        block_header: &BlockHeader,
    ) -> Result<Signature, EncodeError>;

    fn compute_proof_of_work(
        &mut self,
        baker: SignaturePublicKey,
        header: FullHeader,
        proof_of_work_threshold: u64,
    ) -> RequestId;
}

struct Channel<T> {
    pub sender: mpsc::Sender<T>,
    pub receiver: mpsc::Receiver<T>,
}

impl<T> Channel<T> {
    fn new() -> Self {
        let (sender, receiver) = mpsc::channel();
        Self { sender, receiver }
    }
}

pub struct BakerServiceDefault {
    counter: usize,
    bakers: BTreeMap<SignaturePublicKey, BakerSigner>,
    bakers_compute_pow_tasks: BTreeMap<SignaturePublicKey, Arc<RequestId>>,
    worker_channel: Channel<(RequestId, BakerWorkerMessage)>,
}

pub enum BakerSigner {
    Local { secret_key: SecretKeyEd25519 },
}

impl BakerServiceDefault {
    pub fn new() -> Self {
        Self {
            counter: 0,
            bakers: Default::default(),
            bakers_compute_pow_tasks: Default::default(),
            worker_channel: Channel::new(),
        }
    }

    fn new_req_id(&mut self) -> RequestId {
        self.counter = self.counter.wrapping_add(1);
        RequestId::new_unchecked(0, self.counter)
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
    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), mpsc::TryRecvError> {
        self.worker_channel.receiver.try_recv()
    }

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

    fn block_sign(
        &mut self,
        baker: &SignaturePublicKey,
        chain_id: &ChainId,
        block_header: &BlockHeader,
    ) -> Result<Signature, EncodeError> {
        let mut bytes = Vec::new();
        block_header.bin_write(&mut bytes)?;

        self.sign(baker, 0x11, chain_id, bytes.as_ref())
    }

    fn compute_proof_of_work(
        &mut self,
        baker: SignaturePublicKey,
        header: FullHeader,
        proof_of_work_threshold: u64,
    ) -> RequestId {
        let raw_req_id = self.new_req_id();
        let req_id = Arc::new(raw_req_id);
        let weak_req_id = Arc::downgrade(&req_id);
        self.bakers_compute_pow_tasks.insert(baker.clone(), req_id);

        let sender = self.worker_channel.sender.clone();

        std::thread::spawn(move || {
            let cancel = || weak_req_id.strong_count() == 0;
            let nonce = guess_proof_of_work(&header, proof_of_work_threshold, cancel);
            if let Some((req_id, nonce)) =
                nonce.and_then(|nonce| weak_req_id.upgrade().map(|r| (*r, nonce)))
            {
                let _ = sender.send((req_id, BakerWorkerMessage::ComputeProofOfWork(nonce)));
            }
        });

        raw_req_id
    }
}

fn guess_proof_of_work<F>(
    header: &FullHeader,
    proof_of_work_threshold: u64,
    cancel: F,
) -> Option<SizedBytes<8>>
where
    F: Fn() -> bool,
{
    let mut header_bytes = vec![];
    header.bin_write(&mut header_bytes).unwrap();
    let fitness_size = u32::from_be_bytes(header_bytes[78..82].try_into().unwrap()) as usize;
    let nonce_pos = fitness_size + 150;

    loop {
        if cancel() {
            return None;
        }
        for _ in 0..10 {
            let nonce = header_bytes[nonce_pos..(nonce_pos + 8)].try_into().unwrap();
            if check_proof_of_work(&header_bytes, proof_of_work_threshold) {
                return Some(SizedBytes(nonce));
            }

            let nonce = u64::from_be_bytes(nonce).wrapping_add(1).to_be_bytes();
            header_bytes[nonce_pos..(nonce_pos + 8)].clone_from_slice(&nonce);
        }
    }
}

fn check_proof_of_work(header_bytes: &[u8], proof_of_work_threshold: u64) -> bool {
    let hash = blake2b::digest_256(&header_bytes).unwrap();
    let stamp = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    stamp < proof_of_work_threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use crypto::hash::{BlockHash, BlockPayloadHash, ContextHash, OperationListListHash};
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

    #[test]
    fn pow_test() {
        let proof_of_work_threshold = 70368744177663_u64;
        let header = FullHeader {
            level: 232680,
            proto: 2,
            predecessor: BlockHash::from_base58_check(
                "BLu68WtjmwxgPoogFbMXCY1P8gkebaXdBDd1TFAsYjz3vZNyyLv",
            )
            .unwrap(),
            timestamp: chrono::DateTime::parse_from_rfc3339("2022-03-14T10:02:35Z")
                .unwrap()
                .timestamp()
                .into(),
            validation_pass: 4,
            operations_hash: OperationListListHash::from_base58_check(
                "LLoZMEAWjpyMPz19PKzYv2Zbs3kyFDe8XpDzj45wa998ZkCruePZo",
            )
            .unwrap(),
            fitness: vec![
                vec![0x02],
                vec![0x00, 0x03, 0x8c, 0xe8],
                vec![],
                vec![0xff, 0xff, 0xff, 0xff],
                vec![0x00, 0x00, 0x00, 0x00],
            ]
            .into(),
            context: ContextHash::from_base58_check(
                "CoVmcqcynAhio4fodmyNgAcJGKNoyCHPygdBhKGredvUQSjTappc",
            )
            .unwrap(),
            payload_hash: BlockPayloadHash::from_base58_check(
                "vh1mi89F7NNTDQGWLoyhccPzSsuN5RLMAB1EsPdJ9zHZ39XZx39v",
            )
            .unwrap(),
            payload_round: 0,
            proof_of_work_nonce: SizedBytes(
                hex::decode("409a3f3ff9820000").unwrap().try_into().unwrap(),
            ),
            liquidity_baking_escape_vote: false,
            seed_nonce_hash: None,
            signature: Signature(vec![0x00; 64]),
        };
        let mut header_bytes = vec![];
        header.bin_write(&mut header_bytes).unwrap();
        assert!(check_proof_of_work(&header_bytes, proof_of_work_threshold));
    }

    #[test]
    fn pow_fail_test() {
        let proof_of_work_threshold = 70368744177663_u64;
        let header = FullHeader {
            level: 234487,
            proto: 2,
            predecessor: BlockHash::from_base58_check(
                "BKnspHSGAkdjjRQ5k83tPzx73UueJLHmW1jgT4fwTrMyoXUPT2o",
            )
            .unwrap(),
            timestamp: 1647280025.into(),
            validation_pass: 4,
            operations_hash: OperationListListHash::from_base58_check(
                "LLoa6FrQ37toJPdS2PU1cepexNrvncEyqj8D4KtYf1qpccSu1NxSH",
            )
            .unwrap(),
            fitness: vec![
                vec![0x02],
                234487u32.to_be_bytes().to_vec(),
                vec![],
                vec![255, 255, 255, 255],
                vec![0, 0, 0, 0],
            ]
            .into(),
            context: ContextHash::from_base58_check(
                "CoUjRZrB2TVagSRRYJJcc3Y7SUTfSLqZc55EktpYPpfp8xnR6PM8",
            )
            .unwrap(),
            payload_hash: BlockPayloadHash::from_base58_check(
                "vh2VjzHiqaTcKh8wu5AbP6TZ5ibRCqkz7qQ2bAhGK7i2Ffj2akkp",
            )
            .unwrap(),
            payload_round: 0,
            proof_of_work_nonce: SizedBytes([121, 133, 250, 254, 31, 183, 3, 1]),
            liquidity_baking_escape_vote: false,
            seed_nonce_hash: None,
            signature: Signature(vec![0x00; 64]),
        };
        let mut header_bytes = vec![];
        header.bin_write(&mut header_bytes).unwrap();
        assert!(!check_proof_of_work(&header_bytes, proof_of_work_threshold));
    }
}
