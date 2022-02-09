// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::collections::VecDeque;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use tezos_encoding::binary_writer::BinaryWriterError;
use tezos_messages::p2p::encoding::peer::PeerMessageResponse;

use crate::peer::binary_message::write::PeerBinaryMessageWriteState;

// TODO: include error in the state.

#[cfg_attr(fuzzing, derive(fuzzcheck::DefaultMutator))]
#[derive(Error, Serialize, Deserialize, Debug, Clone)]
pub enum PeerMessageWriteError {
    #[error("Error while encoding PeerMessage: {0}")]
    Encode(String),
}

impl From<BinaryWriterError> for PeerMessageWriteError {
    fn from(err: BinaryWriterError) -> Self {
        Self::Encode(err.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PeerMessageWriteState {
    pub queue: VecDeque<Arc<PeerMessageResponse>>,
    pub current: PeerBinaryMessageWriteState,
}
