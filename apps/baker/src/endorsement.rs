// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use thiserror::Error;

use crypto::{
    hash::{BlockHash, ChainId, SecretKeyEd25519},
    CryptoError,
};
use tezos_encoding::enc::{BinError, BinWriter};

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
    pub level: u32,
    pub round: u32,
    pub payload_hash: Vec<u8>,
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
    level: u32,
    round: u32,
    payload_hash: Vec<u8>,
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
    level: u32,
    round: u32,
    payload_hash: Vec<u8>,
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
    level: u32,
    round: u32,
    payload_hash: Vec<u8>,
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
    let op_bytes = {
        let mut v = Vec::with_capacity(75);
        op.bin_write(&mut v)?;
        v
    };
    let watermark_bytes = {
        let mut v = Vec::with_capacity(5);
        watermark.bin_write(&mut v)?;
        v
    };
    let signature = secret_key.sign(&[watermark_bytes.as_slice(), op_bytes.as_slice()])?;

    let mut out = [0; 139];
    out[..75].clone_from_slice(&op_bytes);
    out[75..].clone_from_slice(&signature.0);
    Ok(out)
}
