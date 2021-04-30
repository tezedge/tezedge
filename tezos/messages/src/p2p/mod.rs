// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Provides definitions of p2p messages.

use tezos_encoding::{binary_reader::BinaryReaderError, nom::size};

use self::binary_message::complete_input;

#[macro_use]
pub mod encoding;
pub mod binary_message;

pub fn peer_message_size(bytes: impl AsRef<[u8]>) -> Result<usize, BinaryReaderError> {
    let size = complete_input(size, bytes.as_ref())?;
    Ok(size as usize)
}
