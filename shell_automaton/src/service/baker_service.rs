// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::convert::TryInto;
use std::sync::Arc;

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

use super::service_channel::{
    worker_channel, ResponseTryRecvError, ServiceWorkerRequester, ServiceWorkerResponder,
    ServiceWorkerResponderSender,
};
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
    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), ResponseTryRecvError>;

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
        header: BlockHeader,
        proof_of_work_threshold: u64,
    ) -> RequestId;
}

struct Channel<T> {
    requester: ServiceWorkerRequester<(), T>,
    responder: ServiceWorkerResponder<(), T>,
}

impl<T> Channel<T> {
    fn new(mio_waker: Arc<mio::Waker>) -> Self {
        let (requester, responder) = worker_channel(mio_waker, 1);
        Self {
            requester,
            responder,
        }
    }

    fn try_recv(&mut self) -> Result<T, ResponseTryRecvError> {
        self.requester.try_recv()
    }

    fn responder_sender(&self) -> ServiceWorkerResponderSender<T> {
        self.responder.sender()
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
    pub fn new(mio_waker: Arc<mio::Waker>) -> Self {
        Self {
            counter: 0,
            bakers: Default::default(),
            bakers_compute_pow_tasks: Default::default(),
            worker_channel: Channel::new(mio_waker),
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
    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), ResponseTryRecvError> {
        self.worker_channel.try_recv()
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

        let without_signature_len = bytes.len().saturating_sub(64);
        self.sign(baker, 0x11, chain_id, &bytes[..without_signature_len])
    }

    fn compute_proof_of_work(
        &mut self,
        baker: SignaturePublicKey,
        header: BlockHeader,
        proof_of_work_threshold: u64,
    ) -> RequestId {
        let raw_req_id = self.new_req_id();
        let req_id = Arc::new(raw_req_id);
        let weak_req_id = Arc::downgrade(&req_id);
        self.bakers_compute_pow_tasks.insert(baker.clone(), req_id);

        let mut sender = self.worker_channel.responder_sender();

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
    header: &BlockHeader,
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
}
