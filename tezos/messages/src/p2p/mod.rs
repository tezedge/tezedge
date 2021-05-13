// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

//! Provides definitions of p2p messages.

use tezos_encoding::binary_reader::BinaryReaderError;

#[macro_use]
pub mod encoding;
pub mod binary_message;

pub fn peer_message_size(bytes: impl AsRef<[u8]>) -> Result<usize, BinaryReaderError> {
    let (_, size) = tezos_encoding::nom::size(bytes.as_ref())?;
    Ok(size as usize)
}
