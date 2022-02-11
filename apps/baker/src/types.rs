// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};
use tezos_encoding::encoding::HasEncoding;
use tezos_encoding_derive::NomReader;

use crypto::hash::{
    BlockHash, BlockPayloadHash, ContextHash, NonceHash, OperationListListHash, Signature,
};
use tezos_encoding::enc::BinWriter;

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

#[derive(BinWriter, HasEncoding, NomReader, Serialize, Clone, Debug)]
pub struct ProtocolBlockHeader {
    // pub protocol: ProtocolHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[encoding(sized = "8", bytes)]
    pub proof_of_work_nonce: Vec<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}

// impl ProtocolBlockHeader {
//     pub fn sign(
//         &self,
//         secret_key: &SecretKeyEd25519,
//         chain_id: &ChainId,
//     ) -> Result<Signature, EncodeError> {
//         let (_, signature) = sign_any(secret_key, 0x11, chain_id, self)?;
//         Ok(signature)
//     }
// }
