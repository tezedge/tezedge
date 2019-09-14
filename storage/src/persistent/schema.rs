// Copyright (c) SimpleStaking and Tezos-RS Contributors
// SPDX-License-Identifier: MIT

use failure::Fail;

/// Possible errors for schema
#[derive(Debug, Fail)]
pub enum SchemaError {
    #[fail(display = "Failed to encode value")]
    EncodeError,
    #[fail(display = "Failed to decode value")]
    DecodeError,
}

pub trait Codec: Sized {
    fn decode(bytes: &[u8]) -> Result<Self, SchemaError>;

    fn encode(&self) -> Result<Vec<u8>, SchemaError>;
}

pub trait Schema {
    const COLUMN_FAMILY_NAME: &'static str;

    type Key: Codec;

    type Value: Codec;
}

