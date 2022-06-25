// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::BTreeMap;
use std::convert::TryInto;
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;

use derive_more::From;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crypto::base58::FromBase58CheckError;
use crypto::hash::{
    ChainId, Ed25519Signature, SecretKeyEd25519, SeedEd25519, Signature, TryFromPKError,
};
use crypto::{blake2b, CryptoError, PublicKeyWithHash};
use tezos_encoding::enc::{BinError, BinWriter};
use tezos_encoding::types::SizedBytes;
use tezos_messages::base::signature_public_key::{SignaturePublicKey, SignaturePublicKeyHash};
use tezos_messages::base::ConversionError;
use tezos_messages::p2p::encoding::block_header::BlockHeader;

use super::service_channel::{
    worker_channel, ResponseTryRecvError, ServiceWorkerRequester, ServiceWorkerResponder,
    ServiceWorkerResponderSender,
};
use super::storage_service::StorageError;
use crate::baker::block_endorser::{EndorsementWithForgedBytes, PreendorsementWithForgedBytes};
use crate::baker::persisted::PersistedState;
use crate::request::RequestId;

#[derive(Error, From, Debug)]
pub enum BakerSignError {
    #[error("{_0}")]
    Bin(BinError),
    #[error("{_0}")]
    Crypto(CryptoError),
    #[error("{_0}")]
    Reqwest(reqwest::Error),
    #[error("{_0}")]
    Json(serde_json::Error),
}

pub type BakerSignResult = Result<Signature, BakerSignError>;

#[derive(Debug)]
pub enum BakerWorkerMessage {
    ComputeProofOfWork(SizedBytes<8>),
    PreendorsementSign(BakerSignResult),
    EndorsementSign(BakerSignResult),
    BlockSign(BakerSignResult),
    StateRehydrate(Result<PersistedState, StorageError>),
    StatePersist(Result<(), StorageError>),
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
enum BakerSignTarget {
    Preendorsement,
    Endorsement,
    Block,
}

impl BakerSignTarget {
    fn watermark_tag(&self) -> u8 {
        match self {
            Self::Preendorsement => 0x12,
            Self::Endorsement => 0x13,
            Self::Block => 0x11,
        }
    }

    fn baker_worker_message(self, result: BakerSignResult) -> BakerWorkerMessage {
        match self {
            Self::Preendorsement => BakerWorkerMessage::PreendorsementSign(result),
            Self::Endorsement => BakerWorkerMessage::EndorsementSign(result),
            Self::Block => BakerWorkerMessage::BlockSign(result),
        }
    }
}

pub trait BakerService {
    fn add_local_baker(&mut self, pkh: SignaturePublicKeyHash, secret_key: SecretKeyEd25519);

    fn remove_local_baker(&mut self, pkh: &SignaturePublicKeyHash);

    /// Try to receive/read queued message from baker worker, if there is any.
    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), ResponseTryRecvError>;

    fn preendrosement_sign(
        &mut self,
        baker: &SignaturePublicKeyHash,
        chain_id: &ChainId,
        operation: &PreendorsementWithForgedBytes,
    ) -> RequestId;

    fn endrosement_sign(
        &mut self,
        baker: &SignaturePublicKeyHash,
        chain_id: &ChainId,
        operation: &EndorsementWithForgedBytes,
    ) -> RequestId;

    fn block_sign(
        &mut self,
        baker: &SignaturePublicKeyHash,
        chain_id: &ChainId,
        block_header: &BlockHeader,
    ) -> RequestId;

    fn compute_proof_of_work(
        &mut self,
        baker: SignaturePublicKeyHash,
        header: BlockHeader,
        proof_of_work_threshold: i64,
    ) -> RequestId;

    fn state_rehydrate(&mut self, baker: SignaturePublicKeyHash, chain_id: &ChainId) -> RequestId;

    fn state_persist(
        &mut self,
        baker: SignaturePublicKeyHash,
        chain_id: &ChainId,
        state: PersistedState,
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
    data_dir: PathBuf,
    counter: usize,
    bakers: BTreeMap<SignaturePublicKeyHash, BakerSigner>,
    bakers_compute_pow_tasks: BTreeMap<SignaturePublicKeyHash, Arc<RequestId>>,
    worker_channel: Channel<(RequestId, BakerWorkerMessage)>,
}

#[derive(Debug, Error, From)]
pub enum BakerSignerParseError {
    #[error("schema not found")]
    NoSchema,
    #[error("unknown schema: \"{_0}\"")]
    UnknownSchema(String),
    #[error("only ed25519 keys supported")]
    UnsupportedKey,
    #[error("invalid key {_0}")]
    InvalidKey(FromBase58CheckError),
    #[error("crypto error {_0}")]
    Crypto(CryptoError),
    #[error("public key format error {_0}")]
    PkFormat(TryFromPKError),
    #[error("public key format error {_0}")]
    Conversion(ConversionError),
    #[error("{_0}")]
    InvalidUrl(url::ParseError),
    #[error("missing \"/tz1...\"")]
    MissingPkhPathSegment,
}

#[derive(Clone)]
pub enum BakerSigner {
    Local { secret_key: SecretKeyEd25519 },
    Remote { url: Url },
}

impl BakerSigner {
    pub fn parse_config_str(
        s: &str,
    ) -> Result<(Self, SignaturePublicKeyHash), BakerSignerParseError> {
        let mut it = s.split(':');
        let schema = it.next().ok_or(BakerSignerParseError::NoSchema)?;
        let value = it.next().ok_or(BakerSignerParseError::NoSchema)?;
        match schema {
            "unencrypted" => {
                if !value.starts_with("edsk") {
                    return Err(BakerSignerParseError::UnsupportedKey);
                }
                let (public_key, secret_key) = SeedEd25519::from_base58_check(value)?.keypair()?;

                let signer = BakerSigner::Local { secret_key };
                let pkh = SignaturePublicKey::Ed25519(public_key).pk_hash()?;
                Ok((signer, pkh))
            }
            "http" | "https" => {
                let url = Url::parse(s)?;
                let pkh_str = url
                    .path_segments()
                    .ok_or(BakerSignerParseError::MissingPkhPathSegment)?
                    .last()
                    .ok_or(BakerSignerParseError::MissingPkhPathSegment)?;
                let pkh = SignaturePublicKeyHash::from_b58_hash(pkh_str)?;
                Ok((Self::Remote { url }, pkh))
            }
            s => Err(BakerSignerParseError::UnknownSchema(s.to_string())),
        }
    }
}

impl BakerServiceDefault {
    pub fn new(mio_waker: Arc<mio::Waker>, data_dir: PathBuf) -> Self {
        Self {
            data_dir,
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

    pub fn add_signer(&mut self, pkh: SignaturePublicKeyHash, signer: BakerSigner) {
        self.bakers.insert(pkh, signer);
    }

    pub fn get_baker(&self, key: &SignaturePublicKeyHash) -> &BakerSigner {
        self.bakers
            .get(key)
            .expect("Missing signer for baker: {key}")
    }

    pub fn get_baker_mut(&mut self, key: &SignaturePublicKeyHash) -> &mut BakerSigner {
        self.bakers
            .get_mut(key)
            .expect("Missing signer for baker: {key}")
    }

    fn sign(
        &mut self,
        baker: &SignaturePublicKeyHash,
        target: BakerSignTarget,
        chain_id: &ChainId,
        value_bytes: &[u8],
    ) -> RequestId {
        let req_id = self.new_req_id();

        let watermark_tag = target.watermark_tag();
        let watermark_bytes = Result::<(), BakerSignError>::Ok(()).and_then(|_| {
            let mut v = Vec::with_capacity(5);
            v.push(watermark_tag);
            chain_id.bin_write(&mut v)?;
            Ok(v)
        });
        let value_bytes = value_bytes.to_vec();
        let baker = self.get_baker(baker).clone();

        let mut sender = self.worker_channel.responder_sender();

        std::thread::spawn(move || {
            let result = Ok(()).and_then(|_| match baker {
                BakerSigner::Local { secret_key, .. } => {
                    Ok(secret_key.sign([watermark_bytes?, value_bytes])?)
                }
                BakerSigner::Remote { url, .. } => {
                    let bytes = watermark_bytes?
                        .into_iter()
                        .chain(value_bytes)
                        .collect::<Vec<_>>();
                    let body = format!("{:?}", hex::encode(bytes));
                    let response = reqwest::blocking::Client::new()
                        .post(url)
                        .body(body)
                        .header("Content-Type", "application/json")
                        .send()?;

                    // TODO: refactor Signature to be enum so that it
                    // can represent multiple different signature types.
                    #[derive(Deserialize)]
                    struct SignerResponse {
                        signature: Ed25519Signature,
                    }
                    let SignerResponse { signature } = serde_json::from_reader(response)?;

                    Ok(Signature(signature.0))
                }
            });
            let resp = target.baker_worker_message(result);
            let _ = sender.send((req_id, resp));
        });

        req_id
    }

    fn baker_state_filename(baker: &SignaturePublicKeyHash, chain_id: &ChainId) -> String {
        format!(
            "{}-{}-baker-state.json",
            chain_id.to_base58_check(),
            baker.to_base58_check()
        )
    }

    fn baker_state_filepath(&self, baker: &SignaturePublicKeyHash, chain_id: &ChainId) -> PathBuf {
        self.data_dir
            .join(Self::baker_state_filename(baker, chain_id))
    }
}

impl BakerService for BakerServiceDefault {
    fn add_local_baker(&mut self, pkh: SignaturePublicKeyHash, secret_key: SecretKeyEd25519) {
        self.bakers.insert(pkh, BakerSigner::Local { secret_key });
    }

    fn remove_local_baker(&mut self, pkh: &SignaturePublicKeyHash) {
        self.bakers.remove(pkh);
    }

    fn try_recv(&mut self) -> Result<(RequestId, BakerWorkerMessage), ResponseTryRecvError> {
        self.worker_channel.try_recv()
    }

    fn preendrosement_sign(
        &mut self,
        baker: &SignaturePublicKeyHash,
        chain_id: &ChainId,
        operation: &PreendorsementWithForgedBytes,
    ) -> RequestId {
        let target = BakerSignTarget::Preendorsement;
        self.sign(baker, target, chain_id, operation.forged_with_branch())
    }

    fn endrosement_sign(
        &mut self,
        baker: &SignaturePublicKeyHash,
        chain_id: &ChainId,
        operation: &EndorsementWithForgedBytes,
    ) -> RequestId {
        let target = BakerSignTarget::Endorsement;
        self.sign(baker, target, chain_id, operation.forged_with_branch())
    }

    fn block_sign(
        &mut self,
        baker: &SignaturePublicKeyHash,
        chain_id: &ChainId,
        block_header: &BlockHeader,
    ) -> RequestId {
        let mut bytes = Vec::new();
        block_header.bin_write(&mut bytes).unwrap();

        let without_signature_len = bytes.len().saturating_sub(64);
        let target = BakerSignTarget::Block;
        self.sign(baker, target, chain_id, &bytes[..without_signature_len])
    }

    fn compute_proof_of_work(
        &mut self,
        baker: SignaturePublicKeyHash,
        header: BlockHeader,
        proof_of_work_threshold: i64,
    ) -> RequestId {
        let raw_req_id = self.new_req_id();
        let req_id = Arc::new(raw_req_id);
        let weak_req_id = Arc::downgrade(&req_id);
        self.bakers_compute_pow_tasks.insert(baker, req_id);

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

    fn state_rehydrate(&mut self, baker: SignaturePublicKeyHash, chain_id: &ChainId) -> RequestId {
        let req_id = self.new_req_id();
        let mut sender = self.worker_channel.responder_sender();
        let filepath = self.baker_state_filepath(&baker, chain_id);

        std::thread::spawn(move || {
            let res = Result::<_, StorageError>::Ok(()).and_then(|_| {
                use std::io::ErrorKind;
                let file_exists = std::fs::metadata(&filepath)
                    .map(|_| true)
                    .or_else(|error| {
                        if error.kind() == ErrorKind::NotFound {
                            Ok(false)
                        } else {
                            Err(error)
                        }
                    })?;
                if !file_exists {
                    return Ok(PersistedState::default());
                }
                let file = File::open(filepath)?;
                let reader = BufReader::new(file);
                Ok(serde_json::from_reader(reader)?)
            });
            let _ = sender.send((req_id, BakerWorkerMessage::StateRehydrate(res)));
        });

        req_id
    }

    fn state_persist(
        &mut self,
        baker: SignaturePublicKeyHash,
        chain_id: &ChainId,
        state: PersistedState,
    ) -> RequestId {
        let req_id = self.new_req_id();
        let mut sender = self.worker_channel.responder_sender();
        let data_dir = self.data_dir.clone();
        let filename = Self::baker_state_filename(&baker, chain_id);
        let filepath = self.baker_state_filepath(&baker, chain_id);

        std::thread::spawn(move || {
            let res = Result::<_, StorageError>::Ok(()).and_then(|_| {
                let tmp_filepath = data_dir.join(format!(".tmp-{}", filename));
                let mut file = File::create(&tmp_filepath)?;
                {
                    let mut writer = BufWriter::new(&mut file);
                    serde_json::to_writer(&mut writer, &state)?;
                    writer.write_all(b"\n")?;
                    writer.flush()?;
                }
                file.sync_all()?;
                Ok(std::fs::rename(tmp_filepath, filepath)?)
            });
            let _ = sender.send((req_id, BakerWorkerMessage::StatePersist(res)));
        });

        req_id
    }
}

fn guess_proof_of_work<F>(
    header: &BlockHeader,
    proof_of_work_threshold: i64,
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

fn check_proof_of_work(header_bytes: &[u8], proof_of_work_threshold: i64) -> bool {
    let proof_of_work_threshold = if let Ok(v) = u64::try_from(proof_of_work_threshold) {
        v
    } else {
        return true;
    };
    let hash = blake2b::digest_256(header_bytes).unwrap();
    let stamp = u64::from_be_bytes(hash[0..8].try_into().unwrap());
    stamp < proof_of_work_threshold
}

#[cfg(test)]
mod tests {
    use super::*;

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
