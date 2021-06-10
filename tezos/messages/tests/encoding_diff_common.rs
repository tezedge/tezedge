// Copyright (c) SimpleStaking, Viable Systems and Tezedge Contributors
// SPDX-License-Identifier: MIT

#![allow(dead_code)]

use tezos_encoding::enc::{BinError, BinWriter};

pub fn encode_bin<T: BinWriter>(msg: &T) -> Result<Vec<u8>, BinError> {
    let mut result = Vec::new();
    msg.bin_write(&mut result).map(|_| result)
}

pub fn diff_encodings<T>(msg: &T)
where
    T: BinWriter,
{
    let _ = encode_bin(msg);
}

pub fn diff_encodings_with_error<T>(msg: &T) -> Result<(), BinError>
where
    T: BinWriter,
{
    encode_bin(msg)?;
    Ok(())
}
