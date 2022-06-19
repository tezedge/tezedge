// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crypto::hash::{
    BlockHash, BlockPayloadHash, ContextHash, NonceHash, OperationListListHash, Signature,
};
use tezos_encoding::{
    binary_reader::BinaryReaderError, binary_writer::BinaryWriterError, enc::BinWriter,
    encoding::HasEncoding, nom::NomReader, types::SizedBytes,
};

use crate::{
    p2p::{
        binary_message::{BinaryRead, BinaryWrite},
        encoding::{
            block_header::{BlockHeader as ShellHeader, Level},
            fitness::Fitness,
        },
    },
    Timestamp,
};

#[cfg(feature = "fuzzing")]
use tezos_encoding::fuzzing::sizedbytes::SizedBytesMutator;

#[cfg_attr(feature = "fuzzing", derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone, HasEncoding, NomReader, BinWriter)]
pub struct BlockHeader {
    #[encoding(builtin = "Int32")]
    pub level: Level,
    pub proto: u8,
    pub predecessor: BlockHash,
    pub timestamp: Timestamp,
    pub validation_pass: u8,
    pub operations_hash: OperationListListHash,
    pub fitness: Fitness,
    pub context: ContextHash,
    pub payload_hash: BlockPayloadHash,
    pub payload_round: i32,
    #[cfg_attr(feature = "fuzzing", field_mutator(SizedBytesMutator<8>))]
    pub proof_of_work_nonce: SizedBytes<8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_nonce_hash: Option<NonceHash>,
    pub liquidity_baking_escape_vote: bool,
    pub signature: Signature,
}

#[derive(Debug, thiserror::Error)]
pub enum FromShellHeaderError {
    #[error(transparent)]
    Reader(#[from] BinaryReaderError),
    #[error(transparent)]
    Writer(#[from] BinaryWriterError),
}

impl TryFrom<&ShellHeader> for BlockHeader {
    type Error = FromShellHeaderError;

    fn try_from(shell_header: &ShellHeader) -> Result<Self, Self::Error> {
        let bytes = shell_header.as_bytes()?;
        let header = BlockHeader::from_bytes(&bytes)?;
        Ok(header)
    }
}

impl TryFrom<ShellHeader> for BlockHeader {
    type Error = FromShellHeaderError;

    fn try_from(shell_header: ShellHeader) -> Result<Self, Self::Error> {
        Self::try_from(&shell_header)
    }
}
