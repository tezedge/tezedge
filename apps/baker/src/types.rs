// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use derive_more::From;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crypto::{
    hash::{
        BlockHash, BlockPayloadHash, ChainId, ContextHash, NonceHash, OperationListListHash,
        ProtocolHash, SecretKeyEd25519, Signature,
    },
    CryptoError,
};
use tezos_encoding::enc::{BinError, BinWriter};

#[derive(Deserialize, Serialize)]
pub struct ShellBlockHeader {
    pub level: i32,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: String,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Vec<String>,
    pub context: ContextHash,
}

#[derive(BinWriter, Serialize, Clone)]
pub struct ProtocolBlockHeader {
    pub protocol: ProtocolHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    pub proof_of_work_nonce: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_escape_vote: bool,
}

impl ProtocolBlockHeader {
    pub fn sign(
        &self,
        secret_key: &SecretKeyEd25519,
        chain_id: &ChainId,
    ) -> Result<Signature, EncodeError> {
        let (_, signature) = sign_any(secret_key, 0x11, chain_id, self)?;
        Ok(signature)
    }
}

#[derive(Debug, Error, From)]
pub enum EncodeError {
    #[error("{_0}")]
    Bin(BinError),
    #[error("{_0}")]
    Crypto(CryptoError),
}

pub fn sign_any<T>(
    secret_key: &SecretKeyEd25519,
    watermark_tag: u8,
    chain_id: &ChainId,
    value: &T,
) -> Result<(Vec<u8>, Signature), EncodeError>
where
    T: BinWriter,
{
    let mut v = Vec::new();
    let mut value_bytes = {
        value.bin_write(&mut v)?;
        v
    };
    let watermark_bytes = {
        let mut v = Vec::with_capacity(5);
        v.push(watermark_tag);
        chain_id.bin_write(&mut v)?;
        v
    };
    let signature = secret_key.sign(&[watermark_bytes.as_slice(), value_bytes.as_slice()])?;
    value_bytes.extend_from_slice(&signature.0);
    Ok((value_bytes, signature))
}
