// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use serde::{Deserialize, Serialize};

use crate::peer::chunk::write::{PeerChunkWrite, PeerChunkWriteError, WriteCrypto};

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerBinaryMessageWriteError {
    Chunk(PeerChunkWriteError),
}

impl From<PeerChunkWriteError> for PeerBinaryMessageWriteError {
    fn from(error: PeerChunkWriteError) -> Self {
        Self::Chunk(error)
    }
}

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum PeerBinaryMessageWriteState {
    Init {
        crypto: WriteCrypto,
    },
    Pending {
        chunk_content: Vec<u8>,
        rest_of_message_content: Vec<u8>,
        chunk: PeerChunkWrite,
    },
    Ready {
        crypto: WriteCrypto,
    },
    Error {
        error: PeerBinaryMessageWriteError,
    },
}
