// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::Serialize;
use thiserror::Error;

use crypto::{
    hash::{
        BlockHash, BlockPayloadHash, ChainId, ContextHash, NonceHash, OperationListListHash,
        ProtocolHash, SecretKeyEd25519, Signature,
    },
    CryptoError,
};
use tezos_encoding::enc::{BinError, BinWriter};

pub const BLOCK_HEADER_FITNESS_MAX_SIZE: usize = 0x1000;

type Fitness = Vec<Vec<u8>>;

#[derive(BinWriter)]
pub struct FullHeader {
    #[encoding(builtin = "Int32")]
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    #[encoding(timestamp)]
    pub timestamp: i64,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    #[encoding(composite(
        dynamic = "BLOCK_HEADER_FITNESS_MAX_SIZE",
        list,
        dynamic,
        list,
        builtin = "Uint8"
    ))]
    pub fitness: Fitness,
    pub context: ContextHash,
    pub priority: u16,
    #[encoding(sized = "8", bytes)]
    pub proof_of_work_nonce: Vec<u8>,
    #[encoding(option, sized = "32", bytes)]
    pub seed_nonce_hash: Option<Vec<u8>>,
    pub liquidity_baking_escape_vote: bool,
}

#[derive(BinWriter, Serialize)]
pub struct ProtocolBlockHeader {
    pub protocol: ProtocolHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: u32,
    pub proof_of_work_nonce: Vec<u8>,
    pub seed_nonce_hash: NonceHash,
    pub liquidity_baking_escape_vote: bool,
}

impl ProtocolBlockHeader {
    pub fn sign(
        &self,
        secret_key: &SecretKeyEd25519,
        chain_id: &ChainId,
    ) -> Result<Signature, EncodeError> {
        #[derive(BinWriter)]
        struct Watermark {
            magic: u8,
            chain_id: ChainId,
        }

        let watermark = Watermark {
            magic: 0x11,
            chain_id: chain_id.clone(),
        };

        let (_, signature) = sign_any(secret_key, &watermark, self)?;
        Ok(signature)
    }
}

#[derive(BinWriter)]
#[encoding(tags = "u8")]
pub enum EndorsementKind {
    #[encoding(tag = 0x14)]
    Preendorsement,
    #[encoding(tag = 0x15)]
    Endorsement,
}

#[derive(BinWriter)]
pub struct EndorsementOperation {
    pub branch: BlockHash,
    pub kind: EndorsementKind,
    pub slot: u16,
    pub level: i32,
    pub round: u32,
    pub payload_hash: BlockPayloadHash,
}

#[derive(BinWriter)]
#[encoding(tags = "u8")]
pub enum EndorsementWatermarkKind {
    #[encoding(tag = 0x12)]
    Preendorsement,
    #[encoding(tag = 0x13)]
    Endorsement,
}

#[derive(BinWriter)]
pub struct EndorsementWatermark {
    pub kind: EndorsementWatermarkKind,
    pub chain_id: ChainId,
}

#[derive(Debug, Error, From)]
pub enum EncodeError {
    #[error("{_0}")]
    Bin(BinError),
    #[error("{_0}")]
    Crypto(CryptoError),
}

pub fn generate_preendorsement(
    branch: &BlockHash,
    slot: u16,
    level: i32,
    round: u32,
    payload_hash: BlockPayloadHash,
    chain_id: &ChainId,
    secret_key: &SecretKeyEd25519,
) -> Result<[u8; 139], EncodeError> {
    generate_generic(
        EndorsementKind::Preendorsement,
        branch.clone(),
        slot,
        level,
        round,
        payload_hash,
        chain_id,
        secret_key,
    )
}

pub fn generate_endorsement(
    branch: &BlockHash,
    slot: u16,
    level: i32,
    round: u32,
    payload_hash: BlockPayloadHash,
    chain_id: &ChainId,
    secret_key: &SecretKeyEd25519,
) -> Result<[u8; 139], EncodeError> {
    generate_generic(
        EndorsementKind::Endorsement,
        branch.clone(),
        slot,
        level,
        round,
        payload_hash,
        chain_id,
        secret_key,
    )
}

fn generate_generic(
    kind: EndorsementKind,
    branch: BlockHash,
    slot: u16,
    level: i32,
    round: u32,
    payload_hash: BlockPayloadHash,
    chain_id: &ChainId,
    secret_key: &SecretKeyEd25519,
) -> Result<[u8; 139], EncodeError> {
    let watermark_kind = match &kind {
        &EndorsementKind::Endorsement => EndorsementWatermarkKind::Endorsement,
        &EndorsementKind::Preendorsement => EndorsementWatermarkKind::Preendorsement,
    };
    let op = EndorsementOperation {
        branch,
        kind,
        slot,
        level,
        round,
        payload_hash,
    };
    let watermark = EndorsementWatermark {
        kind: watermark_kind,
        chain_id: chain_id.clone(),
    };

    let (op_bytes, signature) = sign_any(secret_key, &watermark, &op)?;

    let mut out = [0; 139];
    out[..75].clone_from_slice(&op_bytes);
    out[75..].clone_from_slice(&signature.0);
    Ok(out)
}

fn sign_any<W, T>(
    secret_key: &SecretKeyEd25519,
    watermark: &W,
    value: &T,
) -> Result<(Vec<u8>, Signature), EncodeError>
where
    W: BinWriter,
    T: BinWriter,
{
    let value_bytes = {
        let mut v = Vec::new();
        value.bin_write(&mut v)?;
        v
    };
    let watermark_bytes = {
        let mut v = Vec::new();
        watermark.bin_write(&mut v)?;
        v
    };
    let signature = secret_key.sign(&[watermark_bytes.as_slice(), value_bytes.as_slice()])?;
    Ok((value_bytes, signature))
}
