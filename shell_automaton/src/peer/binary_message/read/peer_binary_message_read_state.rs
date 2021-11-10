// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use tezos_encoding::binary_reader::BinaryReaderError;

use crate::peer::chunk::read::{PeerChunkRead, PeerChunkReadError, ReadCrypto};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerBinaryMessageReadError {
    Chunk(PeerChunkReadError),
    Decode(String),
}

impl From<BinaryReaderError> for PeerBinaryMessageReadError {
    fn from(error: BinaryReaderError) -> Self {
        Self::Decode(error.to_string())
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone, strum_macros::AsRefStr)]
pub enum PeerBinaryMessageReadState {
    Init {
        crypto: ReadCrypto,
    },
    PendingFirstChunk {
        chunk: PeerChunkRead,
    },
    Pending {
        buffer: Vec<u8>,
        size: usize,
        chunk: PeerChunkRead,
    },
    Ready {
        crypto: ReadCrypto,
        message: Vec<u8>,
    },
    Error {
        error: PeerBinaryMessageReadError,
    },
}
